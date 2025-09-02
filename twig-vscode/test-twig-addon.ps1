# PowerShell script to build and launch the Twig VSCode extension for local testing

cd c:\code\twig\twig-vscode
npm install
npm run compile

# Launch VSCode in Extension Development Host mode
code --extensionDevelopmentPath=c:\code\twig\twig-vscode
