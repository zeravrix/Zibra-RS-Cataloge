use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

pub struct BackupManager {
    root: PathBuf,
}

impl BackupManager {
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
        }
    }

    pub fn export_backup(&self, target: &Path) -> anyhow::Result<PathBuf> {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = File::create(target)?;
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

        for name in ["zibra_library.sqlite3", "zibra_settings.json"] {
            let p = self.root.join(name);
            if p.is_file() {
                let bytes = fs::read(&p)?;
                zip.start_file(name, options)?;
                zip.write_all(&bytes)?;
            }
        }
        zip.finish()?;
        Ok(target.to_path_buf())
    }

    pub fn import_backup(&self, archive_path: &Path) -> anyhow::Result<usize> {
        let file = File::open(archive_path)?;
        let mut zip = ZipArchive::new(file)?;
        let mut restored = 0usize;
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i)?;
            let name = entry.name().to_string();
            if name == "zibra_library.sqlite3" || name == "zibra_settings.json" {
                let dest = self.root.join(&name);
                let mut buf = Vec::new();
                entry.read_to_end(&mut buf)?;
                fs::write(dest, buf)?;
                restored += 1;
            }
        }
        Ok(restored)
    }
}
