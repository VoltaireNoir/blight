Output demo.gif

Require blight

Set Shell "fish"
Set FontSize 20
Set Padding 30
Set Width 1200
Set Height 600
Set Theme "GruvboxDarkHard"

Type "blight help" Sleep 500ms Enter
Sleep 3s
Ctrl+L
Type "blight setup" Sleep 500ms Enter
Sleep 3s
Ctrl+L
Type "blight status" Sleep 500ms Enter
Sleep 2s
Ctrl+L
Type "blight inc 25 --sweep" Sleep 500ms Enter
Sleep 1s
Type "blight set 50 --device nvidia_0" Sleep 500ms Enter
Sleep 1s
Ctrl+L
Type "blight led" Sleep 500ms Enter
Sleep 3s
Ctrl+L
Type "blight led list" Sleep 500ms Enter
Sleep 3s
Ctrl+L
Type "blight led info (blight led list -f kbd -r)" Sleep 500ms Enter
Sleep 3s
Type "blight led toggle (blight led list -f kbd -r)" Sleep 500ms Enter
Sleep 3s
Ctrl+L

