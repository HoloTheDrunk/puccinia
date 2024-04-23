Output rune.gif

Set Theme "OceanicMaterial"
Set Width 1920
Set Height 1080
Set PlaybackSpeed .5
Set TypingSpeed 100ms

# Write new command
Hide
Type@0ms 'rm ~/.config/puccinia/commands/example.rn'
Enter
Ctrl+L
Show

Sleep 1s

Type@0ms "nvim ~/.config/puccinia/commands/example.rn"
Enter

Sleep 2s

Type "i"
Type@0ms "pub fn example(state, args) {}"
Left
Enter
Tab
Type 'args'
Escape

Sleep 3s

Type ":x"
Enter

Sleep 1s

# Link new command
Type "nvim ~/.config/puccinia/config.pucci"
Enter

Sleep 1s

Type "Go"
Type "command pargs = example:"
Enter
Type "  @arg1: Boolean@"
Enter
Type "@arg2: Number@"
Enter
Type "@arg3: String@"
Enter
Type "Print the arguments."
Enter
Type "!command"
Escape
Type "<<"

Sleep 2s

Type ":x"
Enter

Sleep 1s

# Run puccinia
Type "puccinia test.befunge"
Enter

Sleep 1s

Type ":pargs false 2 hello"
Enter

# Reset loop
Sleep 2s
Type ":q"
Enter
Ctrl+L
Sleep 1s

Hide
Type@0ms 'rm ~/.config/puccinia/commands/example.rn'
Enter
Type "nvim ~/.config/puccinia/config.pucci"
Enter
Sleep 1s
Type "GVkkkkkd"
Type ":x"
Enter
