use sieve::{runtime::RuntimeError, Action, Compiler, Event, Input, Runtime};

fn main() {
  let text_script = br#"
    require ["fileinto", "body", "imap4flags"];

    if body :contains "tps" {
        setflag "$tps_reports";
    }
    if header :matches "List-ID" "*<*@*" {
        fileinto "INBOX.lists.${2}"; stop;
    }
    "#;

  // Compile
  let compiler = Compiler::new();
  let script = compiler.compile(text_script).unwrap();

  // Build runtime
  let runtime = Runtime::new();

  // Create filter instance
  let mut instance = runtime.filter(
    br#"From: Sales Mailing List <list-sales@example.org>
To: John Doe <jdoe@example.org>
List-ID: <sales@example.org>
Subject: TPS Reports
We're putting new coversheets on all the TPS reports before they go out now.
So if you could go ahead and try to remember to do that from now on, that'd be great. All right!
"#,
  );
  let mut input = Input::script("my-script", script);

  // Start event loop
  while let Some(result) = instance.run(input) {
    match result {
      Ok(event) => match event {
        Event::IncludeScript { name, optional } => {
          // NOTE: Just for demonstration purposes, script name needs to be validated first.
          if let Ok(bytes) = std::fs::read(name.as_str()) {
            let script = compiler.compile(&bytes).unwrap();
            input = Input::script(name, script);
          } else if optional {
            input = Input::False;
          } else {
            panic!("Script {} not found.", name);
          }
        }
        Event::MailboxExists { .. } => {
          // Return true if the mailbox exists
          input = false.into();
        }
        Event::ListContains { .. } => {
          // Return true if the list(s) contains an entry
          input = false.into();
        }
        Event::DuplicateId { .. } => {
          // Return true if the ID is duplicate
          input = false.into();
        }
        Event::Execute { command, arguments } => {
          println!(
            "Script executed command {:?} with parameters {:?}",
            command, arguments
          );
          input = false.into(); // Report whether the script succeeded
        }
        #[cfg(test)]
        _ => unreachable!(),
      },
      Err(error) => {
        match error {
          RuntimeError::IllegalAction => {
            eprintln!("Script tried allocating more variables than allowed.");
          }
          RuntimeError::TooManyIncludes => {
            eprintln!("Too many included scripts.");
          }
          RuntimeError::InvalidInstruction(instruction) => {
            eprintln!(
              "Invalid instruction {:?} found at {}:{}.",
              instruction.name(),
              instruction.line_num(),
              instruction.line_pos()
            );
          }
          RuntimeError::ScriptErrorMessage(message) => {
            eprintln!("Script called the 'error' function with {:?}", message);
          }
          RuntimeError::CapabilityNotAllowed(capability) => {
            eprintln!(
              "Capability {:?} has been disabled by the administrator.",
              capability
            );
          }
          RuntimeError::CapabilityNotSupported(capability) => {
            eprintln!("Capability {:?} not supported.", capability);
          }
          RuntimeError::OutOfMemory => {
            eprintln!("Script exceeded the configured memory limit.");
          }
          RuntimeError::CPULimitReached => {
            eprintln!("Script exceeded the configured CPU limit.");
          }
        }
        break;
      }
    }
  }

  // Process actions
  for action in instance.get_actions() {
    match action {
      Action::Keep { flags, message_id } => {
        println!(
          "Keep message '{}' with flags {:?}.",
          std::str::from_utf8(instance.get_message(*message_id).unwrap()).unwrap(),
          flags
        );
      }
      Action::Discard => {
        println!("Discard message.")
      }
      Action::Reject { reason } => {
        println!("Reject message with reason {:?}.", reason);
      }
      Action::Ereject { reason } => {
        println!("Ereject message with reason {:?}.", reason);
      }
      Action::FileInto {
        folder,
        flags,
        message_id,
        ..
      } => {
        println!(
          "File message '{}' in folder {:?} with flags {:?}.",
          std::str::from_utf8(instance.get_message(*message_id).unwrap()).unwrap(),
          folder,
          flags
        );
      }
      Action::SendMessage {
        recipient,
        message_id,
        ..
      } => {
        println!(
          "Send message '{}' to {:?}.",
          std::str::from_utf8(instance.get_message(*message_id).unwrap()).unwrap(),
          recipient
        );
      }
      Action::Notify {
        message, method, ..
      } => {
        println!("Notify URI {:?} with message {:?}", method, message);
      }
    }
  }
}
