// EndBASIC
// Copyright 2021 Julio Merino
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License.  You may obtain a copy
// of the License at:
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS, WITHOUT
// WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.  See the
// License for the specific language governing permissions and limitations
// under the License.

//! Commands to interact with the cloud service.

use crate::*;
use async_trait::async_trait;
use endbasic_core::ast::{ArgSep, ExprType};
use endbasic_core::compiler::{
    ArgSepSyntax, RepeatedSyntax, RepeatedTypeSyntax, RequiredValueSyntax, SingularArgSyntax,
};
use endbasic_core::exec::{Error, Machine, Result, Scope};
use endbasic_core::syms::{Callable, CallableMetadata, CallableMetadataBuilder};
use endbasic_core::LineCol;
use endbasic_std::console::{is_narrow, read_line, read_line_secure, refill_and_print, Console};
use endbasic_std::storage::{FileAcls, Storage};
use endbasic_std::strings::parse_boolean;
use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;
use std::str;

/// Category description for all symbols provided by this module.
const CATEGORY: &str = "Cloud access
The EndBASIC service is a cloud service that provides online file sharing across users of \
EndBASIC and the public.
Files that have been shared publicly can be accessed without an account via the cloud:// file \
system scheme.  All you have to do is mount a user's cloud drive and then access the files as you \
would with your own.  For example:
    MOUNT \"X\", \"cloud://user-123\": DIR \"X:\"
To upload files and share them, you need to create an account.  During account creation time, you \
are assigned a unique, persistent drive in which you can store files privately.  You can later \
choose to share individual files with the public or with specific individuals, at which point \
those people will be able to see them by mounting your drive.
If you have any questions or experience any problems while interacting with the cloud service, \
please contact support@endbasic.dev.";

/// The `LOGIN` command.
pub struct LoginCommand {
    metadata: CallableMetadata,
    service: Rc<RefCell<dyn Service>>,
    console: Rc<RefCell<dyn Console>>,
    storage: Rc<RefCell<Storage>>,
}

impl LoginCommand {
    /// Creates a new `LOGIN` command.
    pub fn new(
        service: Rc<RefCell<dyn Service>>,
        console: Rc<RefCell<dyn Console>>,
        storage: Rc<RefCell<Storage>>,
    ) -> Rc<Self> {
        Rc::from(Self {
            metadata: CallableMetadataBuilder::new("LOGIN")
                .with_syntax(&[
                    (
                        &[SingularArgSyntax::RequiredValue(
                            RequiredValueSyntax {
                                name: Cow::Borrowed("username"),
                                vtype: ExprType::Text,
                            },
                            ArgSepSyntax::End,
                        )],
                        None,
                    ),
                    (
                        &[
                            SingularArgSyntax::RequiredValue(
                                RequiredValueSyntax {
                                    name: Cow::Borrowed("username"),
                                    vtype: ExprType::Text,
                                },
                                ArgSepSyntax::Exactly(ArgSep::Long),
                            ),
                            SingularArgSyntax::RequiredValue(
                                RequiredValueSyntax {
                                    name: Cow::Borrowed("password"),
                                    vtype: ExprType::Text,
                                },
                                ArgSepSyntax::End,
                            ),
                        ],
                        None,
                    ),
                ])
                .with_category(CATEGORY)
                .with_description(
                    "Logs into the user's account.
On a successful login, this mounts your personal drive under the CLOUD:/ location, which you can \
access with any other file-related commands.  Using the cloud:// file system scheme, you can mount \
other people's drives with the MOUNT command.
To create an account, use the SIGNUP command.",
                )
                .build(),
            service,
            console,
            storage,
        })
    }

    /// Performs the login workflow against the server.
    async fn do_login(&self, username: &str, password: &str) -> io::Result<()> {
        let response = self.service.borrow_mut().login(username, password).await?;

        {
            let console = &mut *self.console.borrow_mut();
            if !is_narrow(&*console) && !response.motd.is_empty() {
                console.print("")?;
                console.print("----- BEGIN SERVER MOTD -----")?;
                for line in response.motd {
                    refill_and_print(console, [line], "")?;
                }
                console.print("-----  END SERVER MOTD  -----")?;
                console.print("")?;
            }
        }

        let mut storage = self.storage.borrow_mut();
        storage.mount("CLOUD", &format!("cloud://{}", username))?;

        Ok(())
    }
}

#[async_trait(?Send)]
impl Callable for LoginCommand {
    fn metadata(&self) -> &CallableMetadata {
        &self.metadata
    }

    async fn exec(&self, mut scope: Scope<'_>, _machine: &mut Machine) -> Result<()> {
        if self.service.borrow().is_logged_in() {
            return Err(scope.internal_error("Cannot LOGIN again before LOGOUT"));
        }

        let username = scope.pop_string();
        let password = if scope.nargs() == 0 {
            read_line_secure(&mut *self.console.borrow_mut(), "Password: ")
                .await
                .map_err(|e| scope.io_error(e))?
        } else {
            debug_assert_eq!(1, scope.nargs());
            scope.pop_string()
        };

        self.do_login(&username, &password).await.map_err(|e| scope.io_error(e))
    }
}

/// The `LOGOUT` command.
pub struct LogoutCommand {
    metadata: CallableMetadata,
    service: Rc<RefCell<dyn Service>>,
    console: Rc<RefCell<dyn Console>>,
    storage: Rc<RefCell<Storage>>,
}

impl LogoutCommand {
    /// Creates a new `LOGOUT` command.
    pub fn new(
        service: Rc<RefCell<dyn Service>>,
        console: Rc<RefCell<dyn Console>>,
        storage: Rc<RefCell<Storage>>,
    ) -> Rc<Self> {
        Rc::from(Self {
            metadata: CallableMetadataBuilder::new("LOGOUT")
                .with_syntax(&[(&[], None)])
                .with_category(CATEGORY)
                .with_description(
                    "Logs the user out of their account.
Unmounts the CLOUD drive that was mounted by the LOGIN command.  As a consequence of this, running \
LOGOUT from within the CLOUD drive will fail.",
                )
                .build(),
            service,
            console,
            storage,
        })
    }
}

#[async_trait(?Send)]
impl Callable for LogoutCommand {
    fn metadata(&self) -> &CallableMetadata {
        &self.metadata
    }

    async fn exec(&self, scope: Scope<'_>, _machine: &mut Machine) -> Result<()> {
        debug_assert_eq!(0, scope.nargs());

        if !self.service.borrow().is_logged_in() {
            // TODO(jmmv): Now that the access tokens are part of the service, we can easily allow
            // logging in more than once within a session.  Consider adding a LOGOUT command first
            // to make it easier to handle the CLOUD: drive on a second login.
            return Err(scope.internal_error("Must LOGIN first"));
        }

        let unmounted = match self.storage.borrow_mut().unmount("CLOUD") {
            Ok(()) => true,
            Err(e) if e.kind() == io::ErrorKind::NotFound => false,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                return Err(scope.internal_error("Cannot log out while the CLOUD drive is active"));
            }
            Err(e) => {
                return Err(
                    scope.io_error(io::Error::new(e.kind(), format!("Cannot log out: {}", e)))
                )
            }
        };

        self.service.borrow_mut().logout().await.map_err(|e| scope.io_error(e))?;

        {
            let mut console = self.console.borrow_mut();
            console.print("").map_err(|e| scope.io_error(e))?;
            if unmounted {
                console.print("    Unmounted CLOUD drive").map_err(|e| scope.io_error(e))?;
            }
            console.print("    Good bye!").map_err(|e| scope.io_error(e))?;
            console.print("").map_err(|e| scope.io_error(e))?;
        }

        Ok(())
    }
}

/// The `SHARE` command.
///
/// Note that this command is not exclusively for use by the cloud drive as this interacts with the
/// generic storage layer.  As a result, one might say that this command belongs where other disk
/// commands such as `DIR` are defined, but given that ACLs are primarily a cloud concept in our
/// case, it makes sense to keep it here.
pub struct ShareCommand {
    metadata: CallableMetadata,
    service: Rc<RefCell<dyn Service>>,
    console: Rc<RefCell<dyn Console>>,
    storage: Rc<RefCell<Storage>>,
    exec_base_url: String,
}

impl ShareCommand {
    /// Creates a new `SHARE` command.
    pub fn new<S: Into<String>>(
        service: Rc<RefCell<dyn Service>>,
        console: Rc<RefCell<dyn Console>>,
        storage: Rc<RefCell<Storage>>,
        exec_base_url: S,
    ) -> Rc<Self> {
        Rc::from(Self {
            metadata: CallableMetadataBuilder::new("SHARE")
                .with_syntax(&[(
                    &[SingularArgSyntax::RequiredValue(
                        RequiredValueSyntax {
                            name: Cow::Borrowed("filename"),
                            vtype: ExprType::Text,
                        },
                        ArgSepSyntax::Exactly(ArgSep::Long),
                    )],
                    Some(&RepeatedSyntax {
                        name: Cow::Borrowed("acl"),
                        type_syn: RepeatedTypeSyntax::TypedValue(ExprType::Text),
                        sep: ArgSepSyntax::Exactly(ArgSep::Long),
                        require_one: false,
                        allow_missing: false,
                    }),
                )])
                .with_category(CATEGORY)
                .with_description(
                    "Displays or modifies the ACLs of a file.
If given only a filename$, this command prints out the ACLs of the file.
Otherwise, when given a list of ACL changes, applies those changes to the file.  The acl1$ to \
aclN$ arguments are strings of the form \"username+r\" or \"username-r\", where the former adds \
\"username\" to the users allowed to read the file, and the latter removes \"username\" from the \
list of users allowed to read the file.
You can use the special \"public+r\" ACL to share a file with everyone.  These files can be \
auto-run via the web interface using the special URL that the command prints on success.
Note that this command only works for cloud-based drives as it is designed to share files \
among users of the EndBASIC service.",
                )
                .build(),
            service,
            console,
            storage,
            exec_base_url: exec_base_url.into(),
        })
    }
}

impl ShareCommand {
    /// Parses a textual ACL specification and adds it to `add` or `remove.
    fn parse_acl(
        mut acl: String,
        acl_pos: LineCol,
        add: &mut FileAcls,
        remove: &mut FileAcls,
    ) -> Result<()> {
        let change = if acl.len() < 3 { String::new() } else { acl.split_off(acl.len() - 2) };
        let username = acl; // For clarity after splitting off the ACL change request.
        match (username, change.as_str()) {
            (username, "+r") if !username.is_empty() => add.add_reader(username),
            (username, "+R") if !username.is_empty() => add.add_reader(username),
            (username, "-r") if !username.is_empty() => remove.add_reader(username),
            (username, "-R") if !username.is_empty() => remove.add_reader(username),
            (username, change) => {
                return Err(Error::SyntaxError(
                    acl_pos,
                    format!(
                        "Invalid ACL '{}{}': must be of the form \"username+r\" or \"username-r\"",
                        username, change
                    ),
                ))
            }
        }
        Ok(())
    }

    /// Checks if a file is publicly readable by inspecting a set of ACLs.
    fn has_public_acl(acls: &FileAcls) -> bool {
        for reader in acls.readers() {
            if reader.to_lowercase() == "public" {
                return true;
            }
        }
        false
    }

    /// Fetches and prints the ACLs for `filename`.
    async fn show_acls(&self, filename: &str) -> io::Result<()> {
        let acls = self.storage.borrow().get_acls(filename).await?;

        let mut console = self.console.borrow_mut();
        console.print("")?;
        if acls.readers().is_empty() {
            console.print(&format!("    No ACLs on {}", filename))?;
        } else {
            console.print(&format!("    Reader ACLs on {}:", filename))?;
            for acl in acls.readers() {
                console.print(&format!("    {}", acl))?;
            }
        }
        console.print("")
    }
}

#[async_trait(?Send)]
impl Callable for ShareCommand {
    fn metadata(&self) -> &CallableMetadata {
        &self.metadata
    }

    async fn exec(&self, mut scope: Scope<'_>, _machine: &mut Machine) -> Result<()> {
        debug_assert_ne!(0, scope.nargs());
        let filename = scope.pop_string();

        let mut add = FileAcls::default();
        let mut remove = FileAcls::default();
        while scope.nargs() > 0 {
            let (t, pos) = scope.pop_string_with_pos();
            ShareCommand::parse_acl(t, pos, &mut add, &mut remove)?;
        }

        if add.is_empty() && remove.is_empty() {
            return self.show_acls(&filename).await.map_err(|e| scope.io_error(e));
        }

        self.storage
            .borrow_mut()
            .update_acls(&filename, &add, &remove)
            .await
            .map_err(|e| scope.io_error(e))?;

        if Self::has_public_acl(&add) {
            let filename = match filename.split_once('/') {
                Some((_drive, path)) => path,
                None => &filename,
            };

            let mut console = self.console.borrow_mut();
            console.print("").map_err(|e| scope.io_error(e))?;
            refill_and_print(
                &mut *console,
                [
                    "You have made the file publicly readable.  As a result, other people can now \
auto-run your public file by visiting:",
                    &format!(
                        "{}?run={}/{}",
                        self.exec_base_url,
                        self.service
                            .borrow()
                            .logged_in_username()
                            .expect("SHARE can only succeed against logged in cloud drives"),
                        filename
                    ),
                ],
                "    ",
            )
            .map_err(|e| scope.io_error(e))?;
            console.print("").map_err(|e| scope.io_error(e))?;
        }

        Ok(())
    }
}

/// Checks if a password is sufficiently complex and returns an error when it isn't.
fn validate_password_complexity(password: &str) -> std::result::Result<(), &'static str> {
    if password.len() < 8 {
        return Err("Must be at least 8 characters long");
    }

    let mut alphabetic = false;
    let mut numeric = false;
    for ch in password.chars() {
        if ch.is_alphabetic() {
            alphabetic = true;
        } else if ch.is_numeric() {
            numeric = true;
        }
    }

    if !alphabetic || !numeric {
        return Err("Must contain letters and numbers");
    }

    Ok(())
}

/// The `SIGNUP` command.
pub struct SignupCommand {
    metadata: CallableMetadata,
    service: Rc<RefCell<dyn Service>>,
    console: Rc<RefCell<dyn Console>>,
}

impl SignupCommand {
    /// Creates a new `SIGNUP` command.
    pub fn new(service: Rc<RefCell<dyn Service>>, console: Rc<RefCell<dyn Console>>) -> Rc<Self> {
        Rc::from(Self {
            metadata: CallableMetadataBuilder::new("SIGNUP")
                .with_syntax(&[(&[], None)])
                .with_category(CATEGORY)
                .with_description(
                    "Creates a new user account interactively.
This command will ask you for your personal information to create an account in the EndBASIC \
cloud service.  You will be asked for confirmation before proceeding.",
                )
                .build(),
            service,
            console,
        })
    }

    /// Tries to read a boolean value until it is valid.  Returns `default` if the user hits enter.
    async fn read_bool(console: &mut dyn Console, prompt: &str, default: bool) -> io::Result<bool> {
        loop {
            match read_line(console, prompt, "", None).await? {
                s if s.is_empty() => return Ok(default),
                s => match parse_boolean(s.trim_end()) {
                    Ok(b) => return Ok(b),
                    Err(_) => {
                        console.print("Invalid input; try again.")?;
                        continue;
                    }
                },
            }
        }
    }

    /// Tries to get a password from the user until it is valid.
    async fn read_password(console: &mut dyn Console) -> io::Result<String> {
        loop {
            let password = read_line_secure(console, "Password: ").await?;
            match validate_password_complexity(&password) {
                Ok(()) => (),
                Err(e) => {
                    console.print(&format!("Invalid password: {}; try again.", e))?;
                    continue;
                }
            }

            let second_password = read_line_secure(console, "Retype password: ").await?;
            if second_password != password {
                console.print("Passwords do not match; try again.")?;
                continue;
            }

            return Ok(password);
        }
    }
}

#[async_trait(?Send)]
impl Callable for SignupCommand {
    fn metadata(&self) -> &CallableMetadata {
        &self.metadata
    }

    async fn exec(&self, scope: Scope<'_>, _machine: &mut Machine) -> Result<()> {
        debug_assert_eq!(0, scope.nargs());

        let console = &mut *self.console.borrow_mut();
        console.print("").map_err(|e| scope.io_error(e))?;
        refill_and_print(
            console,
            ["Let's gather some information to create your cloud account.",
"You can abort this process at any time by hitting Ctrl+C and you will be given a chance to \
review your inputs before creating the account."],
            "    ",
        ).map_err(|e| scope.io_error(e))?;
        console.print("").map_err(|e| scope.io_error(e))?;

        let username =
            read_line(console, "Username: ", "", None).await.map_err(|e| scope.io_error(e))?;
        let password = Self::read_password(console).await.map_err(|e| scope.io_error(e))?;

        console.print("").map_err(|e| scope.io_error(e))?;
        refill_and_print(
            console,
            [
                "We also need your email address to activate your account.",
                "Your email address will be kept on file in case we have to notify you of \
important service issues and will never be made public.  You will be asked if you want to receive \
promotional email messages (like new release announcements) or not, and your selection here will \
have no adverse impact in the service you receive.",
            ],
            "    ",
        )
        .map_err(|e| scope.io_error(e))?;
        console.print("").map_err(|e| scope.io_error(e))?;

        let email =
            read_line(console, "Email address: ", "", None).await.map_err(|e| scope.io_error(e))?;
        let promotional_email =
            Self::read_bool(console, "Receive promotional email (y/N)? ", false)
                .await
                .map_err(|e| scope.io_error(e))?;

        console.print("").map_err(|e| scope.io_error(e))?;
        refill_and_print(
            console,
            ["We are ready to go. Please review your answers before proceeding."],
            "    ",
        )
        .map_err(|e| scope.io_error(e))?;
        console.print("").map_err(|e| scope.io_error(e))?;

        console.print(&format!("Username: {}", username)).map_err(|e| scope.io_error(e))?;
        console.print(&format!("Email address: {}", email)).map_err(|e| scope.io_error(e))?;
        console
            .print(&format!("Promotional email: {}", if promotional_email { "yes" } else { "no" }))
            .map_err(|e| scope.io_error(e))?;
        let proceed = Self::read_bool(console, "Continue (y/N)? ", false)
            .await
            .map_err(|e| scope.io_error(e))?;
        if !proceed {
            // TODO(jmmv): This should return an error of some form once we have error handling in
            // the language.
            return Ok(());
        }

        let request = SignupRequest { username, password, email, promotional_email };
        self.service.borrow_mut().signup(&request).await.map_err(|e| scope.io_error(e))?;

        console.print("").map_err(|e| scope.io_error(e))?;
        refill_and_print(
            console,
            ["Your account has been created and is pending activation.",
"Check your email now and look for a message from the EndBASIC Service.  Follow the instructions \
in it to activate your account.  Make sure to check your spam folder.",
"Once your account is activated, come back here and use LOGIN to get started!",
"If you encounter any problems, please contact support@endbasic.dev."],
            "    ",
        ).map_err(|e| scope.io_error(e))?;
        console.print("").map_err(|e| scope.io_error(e))?;

        Ok(())
    }
}

/// Adds all remote manipulation commands for `service` to the `machine`, using `console` to
/// display information and `storage` to manipulate the remote drives.
pub fn add_all<S: Into<String>>(
    machine: &mut Machine,
    service: Rc<RefCell<dyn Service>>,
    console: Rc<RefCell<dyn Console>>,
    storage: Rc<RefCell<Storage>>,
    exec_base_url: S,
) {
    storage
        .borrow_mut()
        .register_scheme("cloud", Box::from(CloudDriveFactory::new(service.clone())));

    machine.add_callable(LoginCommand::new(service.clone(), console.clone(), storage.clone()));
    machine.add_callable(LogoutCommand::new(service.clone(), console.clone(), storage.clone()));
    machine.add_callable(ShareCommand::new(
        service.clone(),
        console.clone(),
        storage,
        exec_base_url,
    ));
    machine.add_callable(SignupCommand::new(service, console));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutils::*;
    use endbasic_std::{console::CharsXY, testutils::*};

    #[test]
    fn test_cloud_scheme_always_available() {
        let t = ClientTester::default();
        assert!(t.get_storage().borrow().has_scheme("cloud"));
    }

    #[test]
    fn test_login_ok_with_password() {
        let mut t = ClientTester::default();
        t.get_service().borrow_mut().add_mock_login(
            "the-username",
            "the-password",
            Ok(LoginResponse { access_token: AccessToken::new("random token"), motd: vec![] }),
        );
        assert!(!t.get_storage().borrow().mounted().contains_key("CLOUD"));
        t.run(format!(r#"LOGIN "{}", "{}""#, "the-username", "the-password"))
            .expect_access_token("random token")
            .check();
        assert!(t.get_storage().borrow().mounted().contains_key("CLOUD"));
    }

    #[test]
    fn test_login_ok_ask_password() {
        let t = ClientTester::default();
        t.get_service().borrow_mut().add_mock_login(
            "the-username",
            "the-password",
            Ok(LoginResponse { access_token: AccessToken::new("random token"), motd: vec![] }),
        );
        let storage = t.get_storage();
        assert!(!storage.borrow().mounted().contains_key("CLOUD"));

        t.get_console().borrow_mut().set_interactive(true);
        let mut exp_output =
            vec![CapturedOut::Write("Password: ".to_string()), CapturedOut::SyncNow];
        for _ in 0.."the-password".len() {
            exp_output.push(CapturedOut::Write("*".to_string()));
        }
        exp_output.push(CapturedOut::Print("".to_owned()));

        t.add_input_chars("the-password")
            .add_input_chars("\n")
            .run(format!(r#"LOGIN "{}""#, "the-username"))
            .expect_access_token("random token")
            .expect_output(exp_output)
            .check();

        assert!(storage.borrow().mounted().contains_key("CLOUD"));
    }

    #[test]
    fn test_login_skip_motd_on_narrow_console() {
        let mut t = ClientTester::default();
        t.get_console().borrow_mut().set_size_chars(CharsXY::new(10, 0));
        t.get_service().borrow_mut().add_mock_login(
            "the-username",
            "the-password",
            Ok(LoginResponse {
                access_token: AccessToken::new("random token"),
                motd: vec!["first line".to_owned(), "second line".to_owned()],
            }),
        );
        t.run(format!(r#"LOGIN "{}", "{}""#, "the-username", "the-password"))
            .expect_access_token("random token")
            .check();
    }

    #[test]
    fn test_login_show_motd_on_wide_console() {
        let mut t = ClientTester::default();
        t.get_service().borrow_mut().add_mock_login(
            "the-username",
            "the-password",
            Ok(LoginResponse {
                access_token: AccessToken::new("random token"),
                motd: vec!["first line".to_owned(), "second line".to_owned()],
            }),
        );
        t.run(format!(r#"LOGIN "{}", "{}""#, "the-username", "the-password"))
            .expect_prints([
                "",
                "----- BEGIN SERVER MOTD -----",
                "first line",
                "second line",
                "-----  END SERVER MOTD  -----",
                "",
            ])
            .expect_access_token("random token")
            .check();
    }

    #[test]
    fn test_login_bad_credentials() {
        let mut t = ClientTester::default();
        t.get_service().borrow_mut().add_mock_login(
            "bad-user",
            "the-password",
            Err(io::Error::new(io::ErrorKind::PermissionDenied, "Unknown user")),
        );
        t.run(format!(r#"LOGIN "{}", "{}""#, "bad-user", "the-password"))
            .expect_err("1:1: Unknown user")
            .check();
        t.get_service().borrow_mut().add_mock_login(
            "the-username",
            "bad-password",
            Err(io::Error::new(io::ErrorKind::PermissionDenied, "Invalid password")),
        );
        t.run(format!(r#"LOGIN "{}", "{}""#, "the-username", "bad-password"))
            .expect_err("1:1: Invalid password")
            .check();
        assert!(!t.get_storage().borrow().mounted().contains_key("CLOUD"));
    }

    #[test]
    fn test_login_twice() {
        let mut t = ClientTester::default();
        t.get_service().borrow_mut().add_mock_login(
            "the-username",
            "the-password",
            Ok(LoginResponse { access_token: AccessToken::new("random token"), motd: vec![] }),
        );
        assert!(!t.get_storage().borrow().mounted().contains_key("CLOUD"));
        t.run(r#"LOGIN "the-username", "the-password": LOGIN "a", "b""#)
            .expect_access_token("random token")
            .expect_err("1:39: Cannot LOGIN again before LOGOUT")
            .check();
        assert!(t.get_storage().borrow().mounted().contains_key("CLOUD"));
    }

    #[test]
    fn test_login_errors() {
        client_check_stmt_compilation_err(
            "1:1: LOGIN expected <username$> | <username$, password$>",
            r#"LOGIN"#,
        );
        client_check_stmt_compilation_err(
            "1:1: LOGIN expected <username$> | <username$, password$>",
            r#"LOGIN "a", "b", "c""#,
        );
        client_check_stmt_compilation_err(
            "1:1: LOGIN expected <username$> | <username$, password$>",
            r#"LOGIN , "c""#,
        );
        client_check_stmt_compilation_err(
            "1:1: LOGIN expected <username$> | <username$, password$>",
            r#"LOGIN ;"#,
        );
        client_check_stmt_compilation_err("1:7: expected STRING but found INTEGER", r#"LOGIN 3"#);
        client_check_stmt_compilation_err(
            "1:7: expected STRING but found INTEGER",
            r#"LOGIN 3, "a""#,
        );
        client_check_stmt_compilation_err(
            "1:12: expected STRING but found INTEGER",
            r#"LOGIN "a", 3"#,
        );
    }

    #[tokio::test]
    async fn test_logout_ok_cloud_not_mounted() {
        let mut t = ClientTester::default();
        t.get_service().borrow_mut().do_login().await;
        t.run(r#"LOGOUT"#).expect_prints(["", "    Good bye!", ""]).check();
        assert!(!t.get_storage().borrow().mounted().contains_key("CLOUD"));
    }

    #[tokio::test]
    async fn test_logout_ok_unmount_cloud() {
        let mut t = ClientTester::default();
        t.get_service().borrow_mut().do_login().await;
        t.get_storage().borrow_mut().mount("CLOUD", "memory://").unwrap();
        t.run(r#"LOGOUT"#)
            .expect_prints(["", "    Unmounted CLOUD drive", "    Good bye!", ""])
            .check();
        assert!(!t.get_storage().borrow().mounted().contains_key("CLOUD"));
    }

    #[tokio::test]
    async fn test_logout_cloud_mounted_and_active() {
        let mut t = ClientTester::default();
        t.get_service().borrow_mut().do_login().await;
        t.get_storage().borrow_mut().mount("CLOUD", "memory://").unwrap();
        t.get_storage().borrow_mut().cd("CLOUD:/").unwrap();
        t.run(r#"LOGOUT"#)
            .expect_err("1:1: Cannot log out while the CLOUD drive is active")
            .expect_access_token("$")
            .check();
        assert!(t.get_storage().borrow().mounted().contains_key("CLOUD"));
    }

    #[test]
    fn test_logout_errors() {
        client_check_stmt_compilation_err("1:1: LOGOUT expected no arguments", r#"LOGOUT "a""#);
        client_check_stmt_err("1:1: Must LOGIN first", r#"LOGOUT"#);
    }

    #[test]
    fn test_login_logout_flow_once() {
        let mut t = ClientTester::default();
        t.get_service().borrow_mut().add_mock_login(
            "u1",
            "p1",
            Ok(LoginResponse { access_token: AccessToken::new("token 1"), motd: vec![] }),
        );
        assert!(!t.get_storage().borrow().mounted().contains_key("CLOUD"));
        t.run(r#"LOGIN "u1", "p1": LOGOUT"#)
            .expect_prints(["", "    Unmounted CLOUD drive", "    Good bye!", ""])
            .check();
        assert!(!t.get_storage().borrow().mounted().contains_key("CLOUD"));
    }

    #[test]
    fn test_login_logout_flow_multiple() {
        let mut t = ClientTester::default();
        t.get_service().borrow_mut().add_mock_login(
            "u1",
            "p1",
            Ok(LoginResponse { access_token: AccessToken::new("token 1"), motd: vec![] }),
        );
        t.get_service().borrow_mut().add_mock_login(
            "u2",
            "p2",
            Ok(LoginResponse { access_token: AccessToken::new("token 2"), motd: vec![] }),
        );
        assert!(!t.get_storage().borrow().mounted().contains_key("CLOUD"));
        t.run(r#"LOGIN "u1", "p1": LOGOUT: LOGIN "u2", "p2""#)
            .expect_prints(["", "    Unmounted CLOUD drive", "    Good bye!", ""])
            .expect_access_token("token 2")
            .check();
        assert!(t.get_storage().borrow().mounted().contains_key("CLOUD"));
    }

    #[test]
    fn test_share_parse_acl_ok() {
        let mut add = FileAcls::default();
        let mut remove = FileAcls::default();

        let lc = LineCol { line: 0, col: 0 };

        ShareCommand::parse_acl("user1+r".to_owned(), lc, &mut add, &mut remove).unwrap();
        ShareCommand::parse_acl("user2+R".to_owned(), lc, &mut add, &mut remove).unwrap();
        ShareCommand::parse_acl("X-r".to_owned(), lc, &mut add, &mut remove).unwrap();
        ShareCommand::parse_acl("Y-R".to_owned(), lc, &mut add, &mut remove).unwrap();
        assert_eq!(&["user1".to_owned(), "user2".to_owned()], add.readers());
        assert_eq!(&["X".to_owned(), "Y".to_owned()], remove.readers());
    }

    #[test]
    fn test_share_has_public_acls() {
        let mut acls = FileAcls::default();
        assert!(!ShareCommand::has_public_acl(&acls));
        acls.add_reader("foo");
        assert!(!ShareCommand::has_public_acl(&acls));
        acls.add_reader("PuBlIc");
        assert!(ShareCommand::has_public_acl(&acls));
    }

    #[test]
    fn test_share_parse_acl_errors() {
        let mut add = FileAcls::default().with_readers(["before1".to_owned()]);
        let mut remove = FileAcls::default().with_readers(["before2".to_owned()]);

        for acl in &["", "r", "+r", "-r", "foo+", "bar-"] {
            let err = ShareCommand::parse_acl(
                acl.to_string(),
                LineCol { line: 12, col: 34 },
                &mut add,
                &mut remove,
            )
            .unwrap_err();
            let message = format!("12:34: {:?}", err);
            assert!(message.contains("Invalid ACL"));
            assert!(message.contains(acl));
        }

        assert_eq!(&["before1".to_owned()], add.readers());
        assert_eq!(&["before2".to_owned()], remove.readers());
    }

    #[tokio::test]
    async fn test_share_print_no_acls() {
        let mut t = ClientTester::default();
        t.get_storage().borrow_mut().put("MEMORY:/FOO", b"").await.unwrap();
        t.run(r#"SHARE "MEMORY:/FOO""#)
            .expect_prints(["", "    No ACLs on MEMORY:/FOO", ""])
            .expect_file("MEMORY:/FOO", "")
            .check();
    }

    #[tokio::test]
    async fn test_share_print_some_acls() {
        let mut t = ClientTester::default();
        {
            let storage = t.get_storage();
            let mut storage = storage.borrow_mut();
            storage.put("MEMORY:/FOO", b"").await.unwrap();
            storage
                .update_acls(
                    "MEMORY:/FOO",
                    &FileAcls::default().with_readers(["some".to_owned(), "person".to_owned()]),
                    &FileAcls::default(),
                )
                .await
                .unwrap();
        }
        t.run(r#"SHARE "MEMORY:/FOO""#)
            .expect_prints(["", "    Reader ACLs on MEMORY:/FOO:", "    person", "    some", ""])
            .expect_file("MEMORY:/FOO", "")
            .check();
    }

    #[tokio::test]
    async fn test_share_make_public() {
        let mut t = ClientTester::default();
        t.get_storage().borrow_mut().put("MEMORY:/FOO.BAS", b"").await.unwrap();
        t.get_service().borrow_mut().do_login().await;
        let mut checker = t.run(r#"SHARE "MEMORY:/FOO.BAS", "Public+r""#);
        let output = flatten_output(checker.take_captured_out());
        checker.expect_file("MEMORY:/FOO.BAS", "").expect_access_token("$").check();
        assert!(output.contains("https://repl.example.com/?run=logged-in-username/FOO.BAS"));
    }

    // TODO(jmmv): Add forgotten tests for SHARE modifying ACLs.

    #[test]
    fn test_share_errors() {
        client_check_stmt_compilation_err(
            "1:1: SHARE expected filename$[, acl1$, .., aclN$]",
            r#"SHARE"#,
        );
        client_check_stmt_compilation_err("1:7: expected STRING but found INTEGER", r#"SHARE 1"#);
        client_check_stmt_compilation_err(
            "1:1: SHARE expected filename$[, acl1$, .., aclN$]",
            r#"SHARE , "a""#,
        );
        client_check_stmt_compilation_err(
            "1:1: SHARE expected filename$[, acl1$, .., aclN$]",
            r#"SHARE "a"; "b""#,
        );
        client_check_stmt_compilation_err(
            "1:1: SHARE expected filename$[, acl1$, .., aclN$]",
            r#"SHARE "a", "b"; "c""#,
        );
        client_check_stmt_compilation_err(
            "1:1: SHARE expected filename$[, acl1$, .., aclN$]",
            r#"SHARE "a", , "b""#,
        );
        client_check_stmt_compilation_err(
            "1:12: expected STRING but found INTEGER",
            r#"SHARE "a", 3, "b""#,
        );
        client_check_stmt_err(
            r#"1:12: Invalid ACL 'foobar': must be of the form "username+r" or "username-r""#,
            r#"SHARE "a", "foobar""#,
        );
    }

    #[test]
    fn test_validate_password_complexity_ok() {
        validate_password_complexity("theP4ssword").unwrap();
    }

    #[test]
    fn test_validate_password_complexity_error() {
        validate_password_complexity("a").unwrap_err().contains("8 characters");
        validate_password_complexity("abcdefg").unwrap_err().contains("8 characters");
        validate_password_complexity("long enough").unwrap_err().contains("letters and numbers");
        validate_password_complexity("1234567890").unwrap_err().contains("letters and numbers");
    }

    #[test]
    fn test_signup_ok() {
        let t = ClientTester::default();
        t.get_service().borrow_mut().add_mock_signup(
            SignupRequest {
                username: "the-username".to_owned(),
                password: "theP4ssword".to_owned(),
                email: "some@example.com".to_owned(),
                promotional_email: false,
            },
            Ok(()),
        );
        t.get_console().borrow_mut().set_interactive(true);

        let mut t = t
            .add_input_chars("the-username\n")
            .add_input_chars("theP4ssword\n")
            .add_input_chars("theP4ssword\n")
            .add_input_chars("some@example.com\n")
            .add_input_chars("\n") // Default promotional email answer.
            .add_input_chars("y\n"); // Confirmation.
        let mut c = t.run("SIGNUP".to_owned());
        let output = flatten_output(c.take_captured_out());
        c.check();

        assert!(output.contains("Username: the-username"));
        assert!(output.contains("Email address: some@example.com"));
        assert!(output.contains("Promotional email: no"));
    }

    #[test]
    fn test_signup_ok_with_promotional_email() {
        let t = ClientTester::default();
        t.get_service().borrow_mut().add_mock_signup(
            SignupRequest {
                username: "foobar".to_owned(),
                password: "AnotherPassword5".to_owned(),
                email: "other@example.com".to_owned(),
                promotional_email: true,
            },
            Ok(()),
        );
        t.get_console().borrow_mut().set_interactive(true);

        let mut t = t
            .add_input_chars("foobar\n")
            .add_input_chars("AnotherPassword5\n")
            .add_input_chars("AnotherPassword5\n")
            .add_input_chars("other@example.com\n")
            .add_input_chars("yes\n") // Promotional email answer.
            .add_input_chars("y\n"); // Confirmation.
        let mut c = t.run("SIGNUP".to_owned());
        let output = flatten_output(c.take_captured_out());
        c.check();

        assert!(output.contains("Username: foobar"));
        assert!(output.contains("Email address: other@example.com"));
        assert!(output.contains("Promotional email: yes"));
    }

    #[test]
    fn test_signup_ok_retry_inputs() {
        let t = ClientTester::default();
        t.get_service().borrow_mut().add_mock_signup(
            SignupRequest {
                username: "the-username".to_owned(),
                password: "AnotherPassword7".to_owned(),
                email: "some@example.com".to_owned(),
                promotional_email: false,
            },
            Ok(()),
        );
        t.get_console().borrow_mut().set_interactive(true);

        let mut t = t
            .add_input_chars("the-username\n")
            .add_input_chars("too simple\n") // Password complexity failure.
            .add_input_chars("123456\n") // Password complexity failure.
            .add_input_chars("AnotherPassword7\n")
            .add_input_chars("does not match\n") // Second password doesn't match.
            .add_input_chars("too simple\n") // Password complexity failure.
            .add_input_chars("123456\n") // Password complexity failure.
            .add_input_chars("AnotherPassword7\n")
            .add_input_chars("AnotherPassword7\n")
            .add_input_chars("some@example.com\n")
            .add_input_chars("123\n") // Promotional email answer failure.
            .add_input_chars("n\n") // Promotional email answer.
            .add_input_chars("foo\n") // Confirmation failure.
            .add_input_chars("y\n"); // Confirmation.
        let mut c = t.run("SIGNUP".to_owned());
        let output = flatten_output(c.take_captured_out());
        c.check();

        assert!(output.contains("Invalid input"));
        assert!(output.contains("Invalid password: Must contain"));
        assert!(output.contains("Passwords do not match"));
        assert!(output.contains("Username: the-username"));
        assert!(output.contains("Email address: some@example.com"));
        assert!(output.contains("Promotional email: no"));
    }

    #[test]
    fn test_signup_abort() {
        let t = ClientTester::default();
        t.get_console().borrow_mut().set_interactive(true);

        let mut t = t
            .add_input_chars("the-username\n")
            .add_input_chars("theP4ssword\n")
            .add_input_chars("theP4ssword\n")
            .add_input_chars("some@example.com\n")
            .add_input_chars("\n") // Default promotional email answer.
            .add_input_chars("\n"); // Default confirmation.
        let mut c = t.run("SIGNUP".to_owned());
        let output = flatten_output(c.take_captured_out());
        c.check();

        assert!(output.contains("Username: the-username"));
        assert!(output.contains("Email address: some@example.com"));
        assert!(output.contains("Promotional email: no"));
    }

    #[test]
    fn test_singup_errors() {
        client_check_stmt_compilation_err("1:1: SIGNUP expected no arguments", r#"SIGNUP "a""#);
    }

    #[test]
    fn test_signup_process_error() {
        let t = ClientTester::default();
        t.get_service().borrow_mut().add_mock_signup(
            SignupRequest {
                username: "the-username".to_owned(),
                password: "theP4ssword".to_owned(),
                email: "some@example.com".to_owned(),
                promotional_email: false,
            },
            Err(io::Error::new(io::ErrorKind::AlreadyExists, "Some error")),
        );
        t.get_console().borrow_mut().set_interactive(true);

        let mut t = t
            .add_input_chars("the-username\n")
            .add_input_chars("theP4ssword\n")
            .add_input_chars("theP4ssword\n")
            .add_input_chars("some@example.com\n")
            .add_input_chars("\n") // Default promotional email answer.
            .add_input_chars("true\n"); // Confirmation.
        let mut c = t.run("SIGNUP".to_owned());
        let output = flatten_output(c.take_captured_out());
        c.expect_err("1:1: Some error").check();

        assert!(output.contains("Username: the-username"));
        assert!(output.contains("Email address: some@example.com"));
        assert!(output.contains("Promotional email: no"));
    }
}
