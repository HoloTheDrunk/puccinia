use self::config::CommandBindingArg;

use super::prelude::*;

use command::config::CommandBinding;
use rune::{alloc::try_format, ToValue, Value};

use crate::grid::span2d;

use {
    itertools::Itertools,
    rune::{Any, Source, Sources},
    walkdir::WalkDir,
};

pub struct Interactions {
    pub commands: Vec<Command>,
    pub properties: Vec<Property>,
}

pub struct Command {
    pub names: Vec<String>,
    pub args: Vec<Arg>,
    pub description: Option<String>,
    pub handler: Box<
        dyn Fn(
            Vec<String>,
            &mut State,
            &Interactions,
            &Sender<logic::Message>,
        ) -> FrontendResult<bool>,
    >,
}

impl ToString for Command {
    fn to_string(&self) -> String {
        let names = self.names.join("|");
        let args = self.args.iter().map(ToString::to_string).join(" ");
        format!(
            "{bind}{}{args}: {description}",
            ["", " "][(args.len() > 0) as usize],
            bind = names,
            args = args,
            description = self
                .description
                .as_ref()
                .map(String::as_str)
                .unwrap_or("No description provided")
        )
    }
}

impl Command {
    pub fn detail(&self) -> String {
        let header = format!(
            "# {names}{}{description}\n\nArguments:\n",
            ["", ": "][self.description.is_some() as usize],
            names = self.names.join("|"),
            description = self.description.clone().unwrap_or("".to_owned())
        );

        let args = self.args.iter().map(|arg| {
            format!(
                "\t- {name}: {ty}{optional}{description}",
                name = arg.name,
                ty = arg.arg_type.to_string(),
                optional = ["", "?"][arg.optional as usize],
                description = ""
            )
        });

        std::iter::once(header).chain(args).join("\n")
    }
}

pub struct Property {
    pub name: &'static str,
    pub args: Vec<Arg>,
    pub description: &'static str,
    pub setter: Box<dyn Fn(&[String], &mut State, &Sender<logic::Message>) -> FrontendResult<()>>,
}

impl ToString for Property {
    fn to_string(&self) -> String {
        let args = self.args.iter().map(ToString::to_string).join(" ");
        format!(
            "{}{}{}: {}",
            self.name,
            ["", " "][(args.len() > 0) as usize],
            args,
            self.description
        )
    }
}

#[derive(Clone)]
pub struct Arg {
    pub name: String,
    pub optional: bool,
    pub arg_type: ArgType,
    pub description: Option<String>,
}

impl From<CommandBindingArg> for Arg {
    fn from(
        CommandBindingArg {
            name,
            arg_type,
            description,
        }: CommandBindingArg,
    ) -> Self {
        Arg {
            name,
            optional: false,
            arg_type,
            description,
        }
    }
}

impl ToString for Arg {
    fn to_string(&self) -> String {
        let surround = [('<', '>'), ('[', ']')][self.optional as usize];
        format!(
            "{}{}:{}{}",
            surround.0,
            self.name,
            self.arg_type.to_string(),
            surround.1
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Any)]
pub enum ArgType {
    #[rune(constructor)]
    String,
    #[rune(constructor)]
    Number,
    #[rune(constructor)]
    Boolean,
    #[rune(constructor)]
    Axis,
}

impl From<&str> for ArgType {
    fn from(value: &str) -> Self {
        if value.parse::<f64>().is_ok() {
            ArgType::Number
        } else if value.parse::<bool>().is_ok() {
            ArgType::Boolean
        } else if value == "x" || value == "y" {
            ArgType::Axis
        } else {
            ArgType::String
        }
    }
}

impl ArgType {
    pub fn is_compatible(&self, other: &Self) -> bool {
        match (self, other) {
            (a, b) if a == b => true,
            (ArgType::String, _) => true,
            (ArgType::Boolean, ArgType::Number) => true,
            _ => false,
        }
    }
}

impl ToString for ArgType {
    fn to_string(&self) -> String {
        match self {
            ArgType::String => "String",
            ArgType::Number => "Number",
            ArgType::Boolean => "Boolean",
            ArgType::Axis => "Axis",
        }
        .to_owned()
    }
}

#[derive(Debug, PartialEq, Eq, Any)]
pub enum Axis {
    X,
    Y,
}

impl TryFrom<char> for Axis {
    type Error = ();

    fn try_from(value: char) -> Result<Self, Self::Error> {
        match value {
            'x' | 'X' => Ok(Axis::X),
            'y' | 'Y' => Ok(Axis::Y),
            _ => Err(()),
        }
    }
}

pub fn load_rune_commands(config_path: Option<&Path>) -> anyhow::Result<Sources> {
    let mut sources = Sources::new();

    if let Some(path) = config_path.map(|p| p.join("commands")) {
        for entry in WalkDir::new(path).into_iter().flatten() {
            let file_name = entry.file_name().to_string_lossy();
            let file_type = entry.file_type();

            if file_type.is_file() && file_name.ends_with(".rn") {
                Source::from_path(entry.path()).map(|source| sources.insert(source))??;
            }
        }
    }

    Ok(sources)
}

fn init_rune_commands(config: &ConfigFile, state: &mut State) -> Vec<Command> {
    todo!()
}

impl From<CommandBinding> for Command {
    fn from(
        CommandBinding {
            bind,
            name,
            args,
            description,
        }: CommandBinding,
    ) -> Self {
        let arg_count = args.len();
        let converted_args = args
            .iter()
            .map(
                |CommandBindingArg {
                     name,
                     arg_type,
                     description,
                 }| Arg {
                    name: name.clone(),
                    optional: false,
                    arg_type: arg_type.clone(),
                    description: description.clone(),
                },
            )
            .collect::<Vec<Arg>>();
        Command {
            names: vec![bind.clone()],
            args: converted_args.clone(),
            description: description.clone(),
            handler: Box::new(move |passed_args, state, _interactions, _sender| {
                if let Some(rune_state) = &state.rune_state {
                    if passed_args.len() != arg_count {
                        state
                            .notifications
                            .push(Notification::new(NotificationContent::Error(Box::new(
                                format!(
                                "Invalid number of arguments, got {} expected {}: {passed_args:?}",
                                passed_args.len(),
                                arg_count
                            ),
                            ))));
                        return Ok(false);
                    }

                    let args = passed_args
                        .into_iter()
                        .enumerate()
                        .map(|(index, arg)| match converted_args[index].arg_type {
                            ArgType::String => arg.to_value().into_result().map(Some),
                            ArgType::Number => {
                                arg.parse::<f32>().map(rune::to_value).ok().transpose()
                            }
                            ArgType::Boolean => {
                                arg.parse::<bool>().map(rune::to_value).ok().transpose()
                            }
                            ArgType::Axis => Axis::try_from(arg.chars().nth(0).unwrap())
                                .map(rune::to_value)
                                .ok()
                                .transpose(),
                        })
                        .map(Result::ok)
                        .map(Option::flatten)
                        .collect::<Vec<Option<Value>>>();

                    let args = if args.iter().any(Option::is_none) {
                        state
                            .notifications
                            .push(Notification::new(NotificationContent::Error(Box::new(
                                format!("Invalid arguments: {args:?}\n\
                                        (`None` indicates a parsing error, you should check the command's expected argument types)"),
                            ))));
                        return Ok(false);
                    } else {
                        args.into_iter().map(Option::unwrap).collect::<Vec<Value>>()
                    };

                    match rune_state
                        .to_vm()
                        .call([name.as_str()], (&mut *state, args))
                    {
                        Ok(result) => {
                            let ty = format!(
                                "{}",
                                result.type_info().into_result().unwrap().to_string()
                            );
                            let result = rune::alloc::fmt::try_format(format_args!("{result:?}"))
                                .map(|s| s.to_string())
                                .unwrap_or("Failed to format rune result".to_owned());

                            state.notifications.push(Notification::new(
                                NotificationContent::CommandResult(format!("{ty}: {result}")),
                            ))
                        }
                        Err(err) => {
                            state.notifications.push(Notification::new(
                                NotificationContent::RuneError(err.to_string()),
                            ));
                        }
                    };
                } else {
                    state
                        .notifications
                        .push(Notification::new(NotificationContent::Error(Box::new(
                            "Runes not loaded",
                        ))));
                }
                Ok(false)
            }),
        }
    }
}

pub fn init_commands(config: &Option<ConfigFile>, state: &mut State) -> Vec<Command> {
    let mut commands = Vec::new();

    if let Some(config) = config.as_ref().filter(|_| state.rune_state.is_some()) {
        let start_time = Instant::now();
        for command_binding in config.commands.iter().cloned() {
            commands.push(Command::from(command_binding));
        }
        let time_taken = Instant::now() - start_time;
        state
            .notifications
            .push(Notification::new(NotificationContent::Info(format!(
                "Loaded Rune commands in {}ms",
                time_taken.as_millis()
            ))));
    }

    macro_rules! svec {
        ($($lit:literal),*) => {
            vec![$($lit.to_owned()),*]
        };
    }

    commands.extend(vec![
        Command {
            names: svec!["q", "quit"],
            args: vec![],
            description: Some("Quit the program".to_owned()),
            handler: Box::new(|_args, _state, _interactions, _sender| Ok(true)),
        },
        Command {
            names: svec!["w", "write"],
            args: vec![Arg {
                name: "path".to_owned(),
                optional: true,
                arg_type: ArgType::String,
                description: Some("Destination file to save into".to_owned()),
            }],
            description: Some("Save the buffer to a given path".to_owned()),
            handler: Box::new(|args, _state, _interactions, sender| {
                let path = args.get(0).map(|s| s.as_str().trim()).unwrap_or("");
                sender
                    .send(logic::Message::Write(
                        (!path.is_empty()).then(|| path.to_owned()),
                    ))
                    .unwrap();
                Ok(false)
            }),
        },
        Command {
            names: svec!["x", "exit"],
            args: vec![Arg {
                name: "path".to_owned(),
                optional: true,
                arg_type: ArgType::String,
                description: Some("Destination path to save into".to_owned()),
            }],
            description: Some("Save the buffer and quit the program".to_owned()),
            handler: Box::new(|args, _state, interactions, sender| {
                (interactions
                    .commands
                    .iter()
                    .find(|c| c.names.contains(&"w".to_owned()))
                    .unwrap()
                    .handler)(args, _state, interactions, sender)?;
                Ok(true)
            }),
        },
        Command {
            names: svec!["t", "trim"],
            args: vec![],
            description: Some("Trim the grid on all sides".to_owned()),
            handler: Box::new(|_args, state, _interactions, _sender| {
                let trimmed = state.grid.trim();

                state.tooltip = Some(Tooltip::Info(format!("{trimmed:?}")));

                if trimmed.iter().any(|v| *v != 0)
                    && !state.grid.check_bounds(state.grid.get_cursor())
                {
                    state.grid.set_cursor(0, 0).unwrap();
                }

                Ok(false)
            }),
        },
        Command {
            names: svec!["r", "run"],
            args: vec![],
            description: Some("Start a run".to_owned()),
            handler: Box::new(|_args, state, _interactions, sender| {
                state.grid.set_cursor(0, 0).unwrap();
                state.grid.set_cursor_dir(Direction::Right);
                state.grid.clear_heat();

                state.stack = Vec::new();
                state.output = String::new();

                state.mode = EditorMode::Running;

                if state.config.run_area_position == RunAreaPosition::Hidden {
                    state.config.run_area_position = RunAreaPosition::Left;
                }

                sender.send(logic::Message::RunningCommand(
                    logic::RunningCommand::Start(state.grid.dump(), state.grid.get_breakpoints()),
                ))?;

                Ok(false)
            }),
        },
        Command {
            names: svec!["s", "set"],
            args: vec![
                Arg {
                    name: "property".to_owned(),
                    optional: false,
                    arg_type: ArgType::String,
                    description: Some("Name of the property to set".to_owned()),
                },
                Arg {
                    name: "value".to_owned(),
                    optional: false,
                    arg_type: ArgType::String,
                    description: None,
                },
            ],
            description: Some("Set a property (use ? for a list)".to_owned()),
            handler: Box::new(|args, state, interactions, sender| {
                handle_set_command(args.as_slice(), state, interactions, sender, true)?;
                Ok(false)
            }),
        },
        Command {
            names: svec!["rev"],
            args: vec![Arg {
                name: "axis".to_owned(),
                optional: true,
                arg_type: ArgType::Axis,
                description: Some("X or Y".to_owned()),
            }],
            description: Some("Reverse selection (horizontally by default)".to_owned()),
            handler: Box::new(|args, state, _interactions, _sender| {
                let Some(EditorMode::Visual(start, end)) = state.previous_mode else {
                    return Err(Error::Command(CommandError::InvalidMode(String::from(
                        "Visual",
                    ))));
                };

                state.push_history();

                let mut buffer = Vec::new();

                // Copy area
                let span = span2d(start, end);

                state.debug = Some(format!("{start:?} {end:?} {span:?}"));

                for y in span.1.clone() {
                    buffer.push(Vec::new());
                    for x in span.0.clone() {
                        buffer.last_mut().unwrap().push(state.grid.get(x, y).value);
                    }
                }

                let axis = args
                    .get(0)
                    .map(|s| s.chars().next())
                    .flatten()
                    .unwrap_or('x');

                match Axis::try_from(axis) {
                    Ok(Axis::X) => {
                        state.grid.loop_over_hv((start, end), |_, y, cell| {
                            cell.value = buffer[y - start.1].pop().unwrap();
                        });
                    }
                    Ok(Axis::Y) => {
                        state.grid.loop_over_hv((start, end), |x, y, cell| {
                            cell.value = buffer[(start.1 as isize - (end.1 as isize)).abs()
                                as usize
                                - (y - start.1)][x - start.0];
                        });
                    }
                    Err(_) => return Err(Error::Command(CommandError::InvalidArguments(args))),
                }

                Ok(false)
            }),
        },
        Command {
            names: svec!["hdump"],
            args: vec![],
            description: Some("Dump the history to the .hist folder".to_owned()),
            handler: Box::new(|_args, state, _interactions, _sender| {
                std::fs::create_dir(".hist").expect("Failed to create .hist folder");
                for i in 0..state.history.inner.len() {
                    let path = format!(".hist/{}", i);
                    std::fs::write(&path, state.history.inner[i].clone())
                        .expect("Failed to write history file");
                }
                Ok(false)
            }),
        },
        Command {
            names: svec!["clear_heat"],
            args: vec![],
            description: Some("Clear the grid's heat".to_owned()),
            handler: Box::new(|_args, state, _interactions, _sender| {
                state.grid.clear_heat();
                Ok(false)
            }),
        },
    ]);

    commands
}

pub fn handle_command(
    cmd: &str,
    state: &mut State,
    interactions: &Interactions,
    sender: &Sender<logic::Message>,
) -> FrontendResult<bool> {
    let (name, args) = cmd.split_once(' ').unwrap_or((cmd, ""));
    let name = name.to_lowercase();
    let commands = &interactions.commands;

    let args = args
        .split(' ')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<String>>();

    if name == "h" || name == "help" {
        state.tooltip = Some(if args.is_empty() || args.iter().all(String::is_empty) {
            Tooltip::Info(
                commands
                    .iter()
                    .map(|c| format!("- {}", c.to_string()))
                    .join("\n"),
            )
        } else if let Some(command) = commands.iter().find(|c| c.names.contains(&args[0])) {
            Tooltip::Info(command.detail())
        } else {
            Tooltip::Error(format!("Command not found: `{}`", args[0]))
        });

        return Ok(false);
    }

    for command in commands.iter() {
        if command.names.contains(&name) {
            // NOTE: Replacing all commands with runes will do this for free
            // TODO: Command arg validation
            // for arg in command.args {
            //     if !arg.arg_type.is_compatible(ArgType::from(arg)) {
            //         state.tooltip = Some(Tooltip::Error(format!(
            //             "Invalid argument type for `{}`: expected {:?}, got {:?}",
            //             arg.name,
            //             arg.arg_type,
            //             ArgType::from(arg)
            //         )));
            //     }
            // }
            return (command.handler)(args, state, interactions, sender);
        }
    }

    state.tooltip = Some(Tooltip::Error(format!("Unknown command `{cmd}`")));

    Ok(false)
}

pub fn init_properties() -> Vec<Property> {
    vec![
        Property {
            name: "heat",
            args: vec![Arg {
                name: "toggle".to_owned(),
                optional: false,
                arg_type: ArgType::Boolean,
                description: None,
            }],
            description: "Heat toggle",
            setter: Box::new(|args, state, _sender| {
                state.config.heat = args[0]
                    .parse()
                    .map_err(|_| Error::Command(CommandError::InvalidArguments(args.to_vec())))?;
                Ok(())
            }),
        },
        Property {
            name: "live_output",
            args: vec![Arg {
                name: "toggle".to_owned(),
                optional: false,
                arg_type: ArgType::Boolean,
                description: None,
            }],
            description: "Live output toggle",
            setter: Box::new(|args, state, _sender| {
                if state.mode == EditorMode::Running {
                    state.tooltip = Some(Tooltip::Error(
                        "Can't change output mode during a run".to_owned(),
                    ));
                } else {
                    state.config.live_output = args[0].parse().map_err(|_| {
                        Error::Command(CommandError::InvalidArguments(args.to_vec()))
                    })?;
                }

                Ok(())
            }),
        },
        Property {
            name: "heat_diffusion",
            args: vec![Arg {
                name: "value".to_owned(),
                optional: false,
                arg_type: ArgType::Number,
                description: None,
            }],
            description: "Heat diffusion per second",
            setter: Box::new(|args, _state, sender| {
                if ArgType::from(args[0].as_ref()) != ArgType::Number {
                    return Err(Error::Command(CommandError::InvalidArguments(
                        args.to_vec(),
                    )));
                }
                update_logic_property("heat_diffusion", &args[0], sender)
            }),
        },
        Property {
            name: "view_updates",
            args: vec![Arg {
                name: "mode".to_owned(),
                optional: false,
                arg_type: ArgType::String,
                description: Some("View update mode (None, Partial, All)".to_owned()),
            }],
            description: "View update mode (None, Partial, All)",
            setter: Box::new(|args, _state, sender| {
                if ArgType::from(args[0].as_ref()) != ArgType::String {
                    return Err(Error::Command(CommandError::InvalidArguments(
                        args.to_vec(),
                    )));
                }
                update_logic_property("view_updates", &args[0], sender)
            }),
        },
        Property {
            name: "step_ms",
            args: vec![Arg {
                name: "value".to_owned(),
                optional: false,
                arg_type: ArgType::Number,
                description: None,
            }],
            description: "Added milliseconds of sleep between steps",
            setter: Box::new(|args, _state, sender| {
                if ArgType::from(args[0].as_ref()) != ArgType::Number {
                    return Err(Error::Command(CommandError::InvalidArguments(
                        args.to_vec(),
                    )));
                }
                update_logic_property("step_ms", &args[0], sender)
            }),
        },
        Property {
            name: "cache_dir",
            args: vec![Arg {
                name: "path".to_owned(),
                optional: false,
                arg_type: ArgType::String,
                description: None,
            }],
            description: "Cache directory path",
            setter: Box::new(|args, state, _sender| {
                state.config.cache_directory = args[0].clone();
                Ok(())
            }),
        },
        Property {
            name: "cache_dir_size",
            args: vec![Arg {
                name: "size".to_owned(),
                optional: false,
                arg_type: ArgType::Number,
                description: None,
            }],
            description: "Cache directory size in MB",
            setter: Box::new(|args, state, _sender| {
                state.config.cache_directory_size = args[0]
                    .parse()
                    .map_err(|_| Error::Command(CommandError::InvalidArguments(args.to_vec())))?;
                Ok(())
            }),
        },
        Property {
            name: "autosave_interval",
            args: vec![Arg {
                name: "interval".to_owned(),
                optional: false,
                arg_type: ArgType::Number,
                description: None,
            }],
            description: "Autosave interval in seconds",
            setter: Box::new(|args, state, _sender| {
                state.config.autosave_interval = args[0]
                    .parse()
                    .map_err(|_| Error::Command(CommandError::InvalidArguments(args.to_vec())))?;
                Ok(())
            }),
        },
    ]
}

fn update_logic_property(
    name: &str,
    value: &str,
    sender: &Sender<logic::Message>,
) -> FrontendResult<()> {
    sender.send(logic::Message::UpdateProperty(
        name.to_owned(),
        value.to_owned(),
    ))?;
    Ok(())
}

pub fn handle_set_command(
    args: &[String],
    state: &mut State,
    interactions: &Interactions,
    sender: &Sender<logic::Message>,
    show_tooltip: bool,
) -> FrontendResult<()> {
    let properties = &interactions.properties;

    let qmark = String::from("?");
    let (name, args) = args.split_first().unwrap_or((&qmark, &[]));

    if name == "?" {
        state.tooltip = Some(Tooltip::Info(
            properties.iter().map(ToString::to_string).join("\n"),
        ));
        return Ok(());
    }

    properties
        .iter()
        .find(|property| property.name == name)
        .map_or_else(
            || {
                Err(Error::Command(CommandError::UnrecognizedProperty(
                    name.clone(),
                )))
            },
            |property| {
                if args.len() < property.args.iter().filter(|arg| !arg.optional).count()
                    || args.len() > property.args.len()
                {
                    return Err(Error::Command(CommandError::InvalidArguments(
                        args.to_vec(),
                    )));
                }

                (property.setter)(args, state, sender)?;
                if show_tooltip {
                    state.tooltip =
                        Some(Tooltip::Info(format!("`{}` has been set", property.name,)));
                }
                Ok(())
            },
        )
}
