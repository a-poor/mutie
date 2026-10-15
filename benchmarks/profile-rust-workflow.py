"""Create an isolated Mutie source copy with workflow timing probes.

Run from the repository root. Prints the new temporary root; does not build or run.
Only tracked files are copied. The source checkout is never edited.
"""
import pathlib,subprocess,tempfile,shutil
repo=pathlib.Path.cwd();root=pathlib.Path(tempfile.mkdtemp(prefix='mutie-workflow-profile-',dir='/private/tmp'));project=root/'project';project.mkdir()
for name in subprocess.check_output(['git','ls-files','-z']).decode().split('\0'):
 if name:
  dst=project/name;dst.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(repo/name,dst)
def replace(file,old,new):
 p=project/file;s=p.read_text();assert s.count(old)==1,(file,old,s.count(old));p.write_text(s.replace(old,new))
replace('src/main.rs','    let mut cli = Cli::parse();','    let _probe = crate::workflow_probe::Span::new("cli_main");\n    let mut cli = Cli::parse();')
replace('src/main.rs','#[tokio::main]','mod workflow_probe;\n\n#[tokio::main]')
(project/'src/workflow_probe.rs').write_text('''use std::{io::Write, time::Instant};
pub struct Span { label: &'static str, started: Instant }
impl Span { pub fn new(label: &'static str) -> Self { Self { label, started: Instant::now() } } }
impl Drop for Span { fn drop(&mut self) { emit(self.label, self.started.elapsed().as_secs_f64()); } }
static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
pub fn emit(label: &str, seconds: f64) {
    let _guard = LOCK.lock().unwrap();
    if let Some(root) = std::env::var_os("MUTIE_PROFILE_DIR") {
        let path = std::path::PathBuf::from(root).join(format!("{}.jsonl",std::process::id()));
        let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path).unwrap();
        writeln!(file,"{}",serde_json::json!({"phase":label,"seconds":seconds})).unwrap();
    }
}
''')
replace('src/saved_report.rs','    fn flush(&mut self) -> Result<()> {','    fn flush(&mut self) -> Result<()> {\n        let _probe = crate::workflow_probe::Span::new("report_flush");')
replace('src/saved_report.rs','    fn write(&self, name: &str, contents: &[u8]) -> Result<()> {','    fn write(&self, name: &str, contents: &[u8]) -> Result<()> {\n        let _probe = crate::workflow_probe::Span::new("report_write");')
replace('src/saved_report.rs','        file.as_file().sync_all()?;','        let probe_started = Instant::now();\n        file.as_file().sync_all()?;\n        crate::workflow_probe::emit("report_sync", probe_started.elapsed().as_secs_f64());')
replace('src/process.rs','        group.shutdown().await;','        crate::workflow_probe::emit("command_to_exit", started.elapsed().as_secs_f64());\n        let probe_started = Instant::now();\n        group.shutdown().await;\n        crate::workflow_probe::emit("group_shutdown", probe_started.elapsed().as_secs_f64());')
replace('src/process.rs','    let elapsed = started.elapsed();','    let elapsed = started.elapsed();\n    crate::workflow_probe::emit("command_total", elapsed.as_secs_f64());')
(root/'logs').mkdir();print(root)
