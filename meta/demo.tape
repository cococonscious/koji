# https://github.com/charmbracelet/vhs

Output meta/demo.gif

Set TypingSpeed 50ms
Set FontSize 22
Set Width 1300
Set Height 800

# Start koji

Type "cd work/earth"
Enter
Sleep 500ms
Type "koji"
Sleep 500ms
Enter
Sleep 1s

# Commit Type

Down
Sleep 250ms
Enter
Sleep 1s

# Scope

Type "body"
Sleep 500ms
Enter
Sleep 1s

# Summary

Type "update math teacher"
Sleep 500ms
Enter
Sleep 1s

# Body

Type "human suit was a bit out of date, should stop these kids from getting suspicious for a while"
Sleep 500ms
Enter
Sleep 1s

# Breaking Change

Type "n"
Sleep 500ms
Enter
Sleep 1s

# Has Open Issue

Type "y"
Sleep 500ms
Enter
Sleep 1s

# Issue Reference

Type "closes #8351"
Sleep 500ms
Enter
Sleep 1s

# Show last commit

Type "git --no-pager show --stat"
Enter

# Hold for 5s at end

Sleep 5s

