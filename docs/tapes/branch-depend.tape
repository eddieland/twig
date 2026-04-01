# Twig Branch Depend — define branch relationships
#
# Generates: docs/demos/branch-depend.gif

Source docs/tapes/setup.tape
Output docs/demos/branch-depend.gif

Hide
Type "source docs/tapes/demo-setup.sh"
Enter
Sleep 2s

# Start fresh — only set up the root, not all the dependencies
# (the setup script already added them, so reset state for this demo)
Type "twig branch remove-dep PROJ-101/auth-middleware main > /dev/null 2>&1; twig branch remove-dep PROJ-102/auth-tests PROJ-101/auth-middleware > /dev/null 2>&1; twig branch remove-dep PROJ-200/api-endpoints main > /dev/null 2>&1"
Enter
Sleep 500ms
Show

# Show tree before — just the root, no hierarchy
Type "twig tree"
Sleep 500ms
Enter
Sleep 2s

# Build the dependency chain
Type "twig branch depend PROJ-101/auth-middleware main"
Sleep 500ms
Enter
Sleep 1s

Type "twig branch depend PROJ-102/auth-tests PROJ-101/auth-middleware"
Sleep 500ms
Enter
Sleep 1s

Type "twig branch depend PROJ-200/api-endpoints main"
Sleep 500ms
Enter
Sleep 1s

# Show tree after — full hierarchy
Type "twig tree"
Sleep 500ms
Enter
Sleep 3s
