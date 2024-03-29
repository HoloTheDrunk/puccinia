use crate::grid::span2d;

use super::prelude::*;

pub struct Interactions {
    pub commands: Vec<Command>,
    pub properties: Vec<Property>,
}

pub struct Command {
    pub names: Vec<&'static str>,
    pub args: Vec<Arg>,
    pub description: &'static str,
    pub handler: Box<
        dyn Fn(Vec<String>, &mut State, &Interactions, &Sender<logic::Message>) -> AnyResult<bool>,
    >,
}

impl ToString for Command {
    fn to_string(&self) -> String {
        let names = self.names.join("|");
        let args = self.args.iter().map(ToString::to_string).join(" ");
        format!(
            "{}{}{}: {}",
            names,
            ["", " "][(args.len() > 0) as usize],
            args,
            self.description
        )
    }
}

pub struct Property {
    pub name: &'static str,
    pub args: Vec<Arg>,
    pub description: &'static str,
    pub setter: Box<dyn Fn(&[String], &mut State, &Sender<logic::Message>) -> AnyResult<()>>,
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

pub struct Arg {
    pub name: &'static str,
    pub optional: bool,
    pub arg_type: ArgType,
}

impl ToString for Arg {
    fn to_string(&self) -> String {
        let surround = [('<', '>'), ('[', ']')][self.optional as usize];
        format!(
            "{}{}:{:?}{}",
            surround.0, self.name, self.arg_type, surround.1
        )
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ArgType {
    String,
    Number,
    Boolean,
    Axis,
    Any,
}

#[derive(Debug, PartialEq, Eq)]
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
    pub fn is_compatible(&self, other: Self) -> bool {
        match self {
            ArgType::String => true,
            ArgType::Number => matches!(other, ArgType::Number),
            ArgType::Boolean => matches!(other, ArgType::Boolean),
            ArgType::Axis => matches!(other, ArgType::Axis),
            ArgType::Any => true,
        }
    }
}

pub fn init_commands() -> Vec<Command> {
    vec![
        Command {
            names: vec!["q", "quit"],
            args: vec![],
            description: "Quit the program",
            handler: Box::new(|_args, _state, _interactions, _sender| Ok(true)),
        },
        Command {
            names: vec!["w", "write"],
            args: vec![Arg {
                name: "path",
                optional: true,
                arg_type: ArgType::String,
            }],
            description: "Save the buffer to a given path",
            handler: Box::new(|args, _state, _interactions, sender| {
                let path = args[0].trim();
                sender
                    .send(logic::Message::Write(
                        (!path.is_empty()).then(|| path.to_owned()),
                    ))
                    .unwrap();
                Ok(false)
            }),
        },
        Command {
            names: vec!["x", "exit"],
            args: vec![Arg {
                name: "path",
                optional: true,
                arg_type: ArgType::String,
            }],
            description: "Save the buffer and quit the program",
            handler: Box::new(|args, _state, _interactions, sender| {
                let path = args[0].trim();
                sender
                    .send(logic::Message::Write(
                        (!path.is_empty()).then(|| path.to_owned()),
                    ))
                    .unwrap();
                Ok(true)
            }),
        },
        Command {
            names: vec!["t", "trim"],
            args: vec![],
            description: "Trim the grid on all sides",
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
            names: vec!["r", "run"],
            args: vec![],
            description: "Start a run",
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
            names: vec!["s", "set"],
            args: vec![
                Arg {
                    name: "property",
                    optional: false,
                    arg_type: ArgType::String,
                },
                Arg {
                    name: "value",
                    optional: false,
                    arg_type: ArgType::Any,
                },
            ],
            description: "Set a property (use ? for a list)",
            handler: Box::new(|args, state, interactions, sender| {
                handle_set_command(args.as_slice(), state, interactions, sender)?;
                Ok(false)
            }),
        },
        Command {
            names: vec!["rev"],
            args: vec![Arg {
                name: "axis",
                optional: true,
                arg_type: ArgType::Axis,
            }],
            description: "Reverse selection (horizontally by default)",
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
                for y in span.1.clone() {
                    buffer.push(Vec::new());
                    for x in span.0.clone() {
                        buffer[y].push(state.grid.get(x, y).value);
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
                            cell.value = buffer[y].pop().unwrap();
                        });
                    }
                    Ok(Axis::Y) => {
                        state.grid.loop_over_hv((start, end), |x, y, cell| {
                            cell.value =
                                buffer[(start.1 as isize - end.1 as isize).abs() as usize - y][x];
                        });
                    }
                    Err(_) => return Err(Error::Command(CommandError::InvalidArguments(args))),
                }

                Ok(false)
            }),
        },
        Command {
            names: vec!["hdump"],
            args: vec![],
            description: "Dump the history to the .hist folder",
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
            names: vec!["clear_heat"],
            args: vec![],
            description: "Clear the grid's heat",
            handler: Box::new(|_args, state, _interactions, _sender| {
                state.grid.clear_heat();
                Ok(false)
            }),
        },
    ]
}

pub fn handle_command(
    cmd: &str,
    state: &mut State,
    interactions: &Interactions,
    sender: &Sender<logic::Message>,
) -> AnyResult<bool> {
    let (name, args) = cmd.split_once(' ').unwrap_or((cmd, ""));
    let name = name.to_lowercase();
    let commands = &interactions.commands;

    if name == "h" || name == "help" {
        state.tooltip = Some(Tooltip::Info(
            commands.iter().map(ToString::to_string).join("\n"),
        ));
        return Ok(false);
    }

    let args = args
        .split(' ')
        .map(str::trim)
        .map(ToString::to_string)
        .collect::<Vec<String>>();

    for command in commands.iter() {
        if command.names.contains(&name.as_ref()) {
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

// TODO: Read property values from a file à-la .vimrc
pub fn init_properties() -> Vec<Property> {
    vec![
        Property {
            name: "heat",
            args: vec![Arg {
                name: "toggle",
                optional: false,
                arg_type: ArgType::Boolean,
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
                name: "toggle",
                optional: false,
                arg_type: ArgType::Boolean,
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
                name: "value",
                optional: false,
                arg_type: ArgType::Number,
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
                name: "mode",
                optional: false,
                arg_type: ArgType::String,
            }],
            description: "View update mode (None, Partial, False)",
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
                name: "value",
                optional: false,
                arg_type: ArgType::Number,
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
    ]
}

fn update_logic_property(
    name: &str,
    value: &str,
    sender: &Sender<logic::Message>,
) -> AnyResult<()> {
    sender.send(logic::Message::UpdateProperty(
        name.to_owned(),
        value.to_owned(),
    ))?;
    Ok(())
}

pub fn handle_set_command(
    cmd: &[String],
    state: &mut State,
    interactions: &Interactions,
    sender: &Sender<logic::Message>,
) -> AnyResult<()> {
    let properties = &interactions.properties;

    let qmark = String::from("?");
    let (name, args) = cmd.split_first().unwrap_or((&qmark, &[]));

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
                state.tooltip = Some(Tooltip::Info(format!("`{}` has been set", property.name,)));
                Ok(())
            },
        )
}
