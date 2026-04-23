# Twig Cascade — rebase a branch stack in sequence
#
# Generates: docs/demos/cascade.gif

Source docs/tapes/setup.tape
Output docs/demos/cascade.gif

Hide
Type "source docs/tapes/demo-setup.sh"
Enter
Sleep 2s

# Add a new commit on the base branch so there's something to cascade
Type "git checkout main --quiet"
Enter
Sleep 500ms
Type "echo 'v2' >> README.md && git add README.md && git commit -m 'docs: update readme' --quiet"
Enter
Sleep 500ms

# Switch to the middle of the stack
Type "git checkout PROJ-101/auth-middleware --quiet"
Enter
Sleep 500ms
Show

Type "twig cascade"
Sleep 500ms
Enter
Sleep 5s
