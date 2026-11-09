//! Scoped command transport: actual owner rules reach real child pipes, never the host kernel.
use super::*;
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::rc::Rc;

type NftSignals = Rc<RefCell<Vec<(usize, std::sync::Arc<std::sync::atomic::AtomicBool>)>>>;

pub(crate) struct Event {
    pub program: String,
    pub args: Vec<String>,
    pub input: PathBuf,
}
pub(crate) struct Network {
    pub root: PathBuf,
    pub events: Rc<RefCell<Vec<Event>>>,
    pub fail_at: Rc<Cell<Option<usize>>>,
    pub signals: NftSignals,
    previous: Option<PathBuf>,
    _spawn: process::SpawnScope,
}
impl Network {
    pub(crate) fn new() -> Self {
        let root = std::env::temp_dir().join(format!("dw-network-{}", Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("ip_forward"), b"1\n").unwrap();
        let previous = TEST_SLOT_ROOT.with(|slot| slot.replace(Some(root.clone())));
        let events = Rc::new(RefCell::new(Vec::new()));
        let recorded = events.clone();
        let fail_at = Rc::new(Cell::new(None));
        let failure = fail_at.clone();
        let directory = root.clone();
        let signals: NftSignals = Rc::new(RefCell::new(Vec::new()));
        let notifications = signals.clone();
        let nft_count = Cell::new(0usize);
        let scope = process::scoped_spawn(move |command| {
            let program = command.get_program().to_string_lossy().into_owned();
            assert!(
                [IP, NFT].contains(&program.as_str()),
                "unexpected network executable"
            );
            let args: Vec<String> = command
                .get_args()
                .map(|v| v.to_string_lossy().into_owned())
                .collect();
            let index = recorded.borrow().len();
            let input = directory.join(format!("input-{index}"));
            let nft = program == NFT || args.iter().any(|arg| arg == NFT);
            recorded.borrow_mut().push(Event {
                program,
                args,
                input: input.clone(),
            });
            if nft {
                let count = nft_count.get() + 1;
                nft_count.set(count);
                for (at, signal) in notifications.borrow().iter() {
                    if count == *at {
                        signal.store(true, std::sync::atomic::Ordering::SeqCst);
                    }
                }
            }
            let script = if failure.get() == Some(index) {
                "cat > \"$1\"; exit 1"
            } else {
                "cat > \"$1\""
            };
            Command::new("/bin/sh")
                .args(["-c", script, "dw-network-fixture"])
                .arg(input)
                .env_clear()
                .stdin(if nft { Stdio::piped() } else { Stdio::null() })
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
        });
        Self {
            root,
            events,
            fail_at,
            signals,
            previous,
            _spawn: scope,
        }
    }
}
impl Drop for Network {
    fn drop(&mut self) {
        TEST_SLOT_ROOT.with(|slot| *slot.borrow_mut() = self.previous.take());
        std::fs::remove_dir_all(&self.root).unwrap();
    }
}
