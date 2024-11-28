use std::{
  io::Error,
  sync::{atomic::AtomicBool, Arc},
};

// A friend of the Signals iterator, but can be customized by what we want yielded about each
// signal.
use signal_hook::iterator::exfiltrator::WithOrigin;
use signal_hook::{
  consts::{signal::*, TERM_SIGNALS},
  flag,
  iterator::SignalsInfo,
  low_level,
};

struct App;
impl App {
  fn run_background() -> Self {
    Self
  }
  fn wait_for_stop(self) {
    println!("wait for stop");
  }
  fn restore_term(&self) {
    println!("restore term");
  }
  fn claim_term(&self) {
    println!("claim term");
  }
  fn resize_term(&self) {
    println!("resize term");
  }
  fn reload_config(&self) {
    println!("reload config");
  }
  fn print_stats(&self) {
    println!("print stats");
  }
}

fn main() -> Result<(), Error> {
  // Make sure double CTRL+C and similar kills
  let term_now = Arc::new(AtomicBool::new(false));
  for sig in TERM_SIGNALS {
    // When terminated by a second term signal, exit with exit code 1.
    // This will do nothing the first time (because term_now is false).
    flag::register_conditional_shutdown(*sig, 1, Arc::clone(&term_now))?;
    // But this will "arm" the above for the second time, by setting it to true.
    // The order of registering these is important, if you put this one first, it will
    // first arm and then terminate ‒ all in the first round.
    flag::register(*sig, Arc::clone(&term_now))?;
  }

  // Subscribe to all these signals with information about where they come from. We use the
  // extra info only for logging in this example (it is not available on all the OSes or at
  // all the occasions anyway, it may return `Unknown`).
  let mut sigs = vec![
    // Some terminal handling
    SIGTSTP, SIGCONT, SIGWINCH,
    // Reload of configuration for daemons ‒ um, is this example for a TUI app or a daemon
    // O:-)? You choose...
    SIGHUP, // Application-specific action, to print some statistics.
    SIGUSR1,
  ];
  sigs.extend(TERM_SIGNALS);
  let mut signals = SignalsInfo::<WithOrigin>::new(&sigs)?;

  // This is the actual application that'll start in its own thread. We'll control it from
  // this thread based on the signals, but it keeps running.
  // This is called after all the signals got registered, to avoid the short race condition
  // in the first registration of each signal in multi-threaded programs.
  let app = App::run_background();

  // Consume all the incoming signals. This happens in "normal" Rust thread, not in the
  // signal handlers. This means that we are allowed to do whatever we like in here, without
  // restrictions, but it also means the kernel believes the signal already got delivered, we
  // handle them in delayed manner. This is in contrast with eg the above
  // `register_conditional_shutdown` where the shutdown happens *inside* the handler.
  let mut has_terminal = true;
  for info in &mut signals {
    // Will print info about signal + where it comes from.
    eprintln!("Received a signal {:?}", info);
    match info.signal {
      SIGTSTP => {
        // Restore the terminal to non-TUI mode
        if has_terminal {
          app.restore_term();
          has_terminal = false;
          // And actually stop ourselves.
          low_level::emulate_default_handler(SIGTSTP)?;
        }
      }
      SIGCONT => {
        if !has_terminal {
          app.claim_term();
          has_terminal = true;
        }
      }
      SIGWINCH => app.resize_term(),
      SIGHUP => app.reload_config(),
      SIGUSR1 => app.print_stats(),
      term_sig => {
        // These are all the ones left
        eprintln!("Terminating");
        assert!(TERM_SIGNALS.contains(&term_sig));
        break;
      }
    }
  }

  // If during this another termination signal comes, the trick at the top would kick in and
  // terminate early. But if it doesn't, the application shuts down gracefully.
  app.wait_for_stop();

  Ok(())
}
