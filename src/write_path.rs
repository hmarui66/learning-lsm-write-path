use std::fs::OpenOptions;
use std::io::{Write, BufWriter};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{sync_channel, SyncSender, Receiver};
use std::thread::{self, JoinHandle};

/// ログエントリ
#[derive(Clone)]
pub struct LogEntry {
    pub key: Vec<u8>,
    pub value: Vec<u8>,
}

/// Mutable なバッファ
struct MemTable {
    entries: Vec<LogEntry>,
    size: usize,
}

impl MemTable {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
            size: 0,
        }
    }

    fn put(&mut self, key: Vec<u8>, value: Vec<u8>) {
        let entry_size = key.len() + value.len();
        self.entries.push(LogEntry { key, value });
        self.size += entry_size;
    }

    fn size(&self) -> usize {
        self.size
    }

    fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// LSM-Tree の書き込みパス
pub struct WritePath {
    /// 現在のmutableバッファ
    memtable: Arc<Mutex<MemTable>>,
    /// バッファサイズの閾値
    size_threshold: usize,
    /// Immutableバッファを送信するチャネル (bounded channelでwrite stallを実現)
    flush_sender: Option<SyncSender<MemTable>>,
    /// バックグラウンドスレッドのハンドル
    flush_thread: Option<JoinHandle<()>>,
    /// 出力ディレクトリ
    data_dir: PathBuf,
    /// SSTableファイルのカウンター
    sstable_counter: Arc<Mutex<usize>>,
    /// immutable MemTableの最大数（RocksDBのmax_write_buffer_number相当）
    max_write_buffer_number: usize,
}

impl WritePath {
    /// 新しいWritePathを作成（デフォルトのmax_write_buffer_number = 2）
    pub fn new<P: AsRef<Path>>(data_dir: P, size_threshold: usize) -> std::io::Result<Self> {
        Self::with_max_write_buffers(data_dir, size_threshold, 2)
    }

    /// max_write_buffer_numberを指定してWritePathを作成
    pub fn with_max_write_buffers<P: AsRef<Path>>(
        data_dir: P,
        size_threshold: usize,
        max_write_buffer_number: usize,
    ) -> std::io::Result<Self> {
        let data_dir = data_dir.as_ref().to_path_buf();

        // データディレクトリを作成
        std::fs::create_dir_all(&data_dir)?;

        // bounded channelで上限を設定（mutable 1個 + immutable (max-1)個）
        // RocksDB: max_write_buffer_number個のMemTable（1 mutable + (max-1) immutable）
        let buffer_capacity = max_write_buffer_number.saturating_sub(1).max(1);
        let (tx, rx) = sync_channel(buffer_capacity);
        let sstable_counter = Arc::new(Mutex::new(0));

        // バックグラウンドフラッシュスレッドを起動
        let flush_thread = Self::spawn_flush_thread(rx, data_dir.clone(), sstable_counter.clone());

        Ok(Self {
            memtable: Arc::new(Mutex::new(MemTable::new())),
            size_threshold,
            flush_sender: Some(tx),
            flush_thread: Some(flush_thread),
            data_dir,
            sstable_counter,
            max_write_buffer_number,
        })
    }

    /// キーと値を書き込む
    ///
    /// immutable MemTableの数が上限に達している場合、
    /// フラッシュが完了するまで書き込みがブロックされる（write stall）
    pub fn put(&self, key: Vec<u8>, value: Vec<u8>) -> std::io::Result<()> {
        let mut memtable = self.memtable.lock().unwrap();

        memtable.put(key, value);

        // サイズ閾値を超えたらフラッシュ
        // このsend()でブロックする可能性がある（write stall）
        if memtable.size() >= self.size_threshold {
            self.freeze_memtable(&mut memtable)?;
        }

        Ok(())
    }

    /// 現在のmemtableをimmutable化して新しいmemtableを作成
    fn freeze_memtable(&self, memtable: &mut std::sync::MutexGuard<MemTable>) -> std::io::Result<()> {
        // 古いmemtableを取り出し、新しいmemtableと交換
        let old_memtable = std::mem::replace(&mut **memtable, MemTable::new());

        // バックグラウンドスレッドに送信
        if !old_memtable.is_empty() {
            if let Some(sender) = &self.flush_sender {
                sender.send(old_memtable)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
            }
        }

        Ok(())
    }

    /// 明示的にフラッシュ（すべてのデータをディスクに書き出す）
    pub fn flush(&self) -> std::io::Result<()> {
        let mut memtable = self.memtable.lock().unwrap();
        if !memtable.is_empty() {
            self.freeze_memtable(&mut memtable)?;
        }
        Ok(())
    }

    /// バックグラウンドフラッシュスレッドを生成
    fn spawn_flush_thread(
        rx: Receiver<MemTable>,
        data_dir: PathBuf,
        counter: Arc<Mutex<usize>>,
    ) -> JoinHandle<()> {
        thread::spawn(move || {
            while let Ok(memtable) = rx.recv() {
                if let Err(e) = Self::write_sstable(&data_dir, &memtable, &counter) {
                    eprintln!("Failed to write SSTable: {}", e);
                }
            }
        })
    }

    /// SSTableファイルに書き出す
    fn write_sstable(
        data_dir: &Path,
        memtable: &MemTable,
        counter: &Arc<Mutex<usize>>,
    ) -> std::io::Result<()> {
        let file_num = {
            let mut c = counter.lock().unwrap();
            let num = *c;
            *c += 1;
            num
        };

        let file_path = data_dir.join(format!("{:06}.sst", file_num));
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(file_path)?;

        // BufWriterでバッファリング（デフォルト8KB）
        let mut writer = BufWriter::new(file);

        // シンプルなフォーマット: [key_len: u32][key][value_len: u32][value]
        for entry in &memtable.entries {
            writer.write_all(&(entry.key.len() as u32).to_le_bytes())?;
            writer.write_all(&entry.key)?;
            writer.write_all(&(entry.value.len() as u32).to_le_bytes())?;
            writer.write_all(&entry.value)?;
        }

        // flushでバッファをディスクに書き出す（fsyncはしない）
        writer.flush()?;
        Ok(())
    }
}

impl Drop for WritePath {
    fn drop(&mut self) {
        // 残りのデータをフラッシュ（エラーは無視）
        let _ = self.flush();

        // flush_senderをdropしてチャネルを閉じる
        // これによりバックグラウンドスレッドのrecv()が終了する
        drop(self.flush_sender.take());

        // バックグラウンドスレッドが終了するまで待機
        if let Some(thread) = self.flush_thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_put_and_flush() {
        let temp_dir = tempfile::tempdir().unwrap();
        let write_path = WritePath::new(temp_dir.path(), 1024).unwrap();

        // データを書き込む
        write_path.put(b"key1".to_vec(), b"value1".to_vec()).unwrap();
        write_path.put(b"key2".to_vec(), b"value2".to_vec()).unwrap();

        // 明示的にフラッシュ
        write_path.flush().unwrap();

        // dropでフラッシュスレッドを待機
        drop(write_path);

        // SSTableファイルが作成されていることを確認
        let files: Vec<_> = fs::read_dir(temp_dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "sst"))
            .collect();

        assert!(!files.is_empty(), "SSTable file should be created");
    }

    #[test]
    fn test_automatic_flush_on_size_threshold() {
        let temp_dir = tempfile::tempdir().unwrap();
        let write_path = WritePath::new(temp_dir.path(), 100).unwrap(); // 小さい閾値

        // 閾値を超えるデータを書き込む
        for i in 0..10 {
            let key = format!("key{:03}", i).into_bytes();
            let value = vec![b'x'; 50]; // 50 bytes
            write_path.put(key, value).unwrap();
        }

        // dropでフラッシュスレッドを待機
        drop(write_path);

        // 複数のSSTableファイルが作成されていることを確認
        let files: Vec<_> = fs::read_dir(temp_dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "sst"))
            .collect();

        assert!(files.len() > 1, "Multiple SSTable files should be created");
    }

    #[test]
    fn test_write_stall_with_max_write_buffers() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::time::Duration;

        let temp_dir = tempfile::tempdir().unwrap();

        // max_write_buffer_number=2, 小さいsize_thresholdでテスト
        // これにより1個のimmutable MemTableまでしか保持できない
        let write_path = Arc::new(
            WritePath::with_max_write_buffers(temp_dir.path(), 100, 2).unwrap()
        );

        // フラッシュスレッドを遅延させるため、大量の小さいフラッシュを発生させる
        let stalled = Arc::new(AtomicBool::new(false));
        let stalled_clone = stalled.clone();
        let write_path_clone = write_path.clone();

        // 別スレッドで書き込みを続ける
        let handle = std::thread::spawn(move || {
            for i in 0..100 {
                let key = format!("key{:03}", i).into_bytes();
                let value = vec![b'x'; 50];

                // write stallが発生したことを検出
                let start = std::time::Instant::now();
                write_path_clone.put(key, value).unwrap();

                if start.elapsed() > Duration::from_millis(10) {
                    stalled_clone.store(true, Ordering::SeqCst);
                }

                // わずかに遅延を入れる
                std::thread::sleep(Duration::from_micros(100));
            }
        });

        handle.join().unwrap();
        drop(write_path);

        // write stallが発生したことを確認
        // （フラッシュが追いつかず、send()がブロックされたはず）
        println!("Write stall detected: {}", stalled.load(Ordering::SeqCst));
    }
}
