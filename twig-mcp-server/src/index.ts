#!/usr/bin/env node

import { Server } from '@modelcontextprotocol/sdk/server/index.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import {
  CallToolRequestSchema,
  ListToolsRequestSchema,
  Tool,
} from '@modelcontextprotocol/sdk/types.js';
import { TwigCLI } from './utils/cli.js';

// Import tool functions
import { getTree } from './tools/tree.js';
import { switchBranch, createBranch, deleteBranch } from './tools/branch.js';
import { addDependency, removeDependency } from './tools/dependency.js';
import { addRootBranch, removeRootBranch, listRootBranches } from './tools/root.js';
import { tidyClean, tidyPrune } from './tools/tidy.js';
import { createPullRequest } from './tools/github.js';

class TwigMCPServer {
  private server: Server;
  private cli: TwigCLI;

  constructor() {
    this.server = new Server(
      {
        name: 'twig-mcp-server',
        version: '1.0.0',
      },
      {
        capabilities: {
          tools: {},
        },
      }
    );

    this.cli = new TwigCLI(process.cwd());
    this.setupHandlers();
  }

  private setupHandlers() {
    // List available tools
    this.server.setRequestHandler(ListToolsRequestSchema, async () => {
      return {
        tools: this.getToolDefinitions(),
      };
    });

    // Handle tool calls
    this.server.setRequestHandler(CallToolRequestSchema, async (request) => {
      const { name, arguments: args } = request.params;

      try {
        switch (name) {
          case 'twig_get_tree':
            return {
              content: [
                {
                  type: 'text',
                  text: JSON.stringify(await getTree(this.cli, args), null, 2),
                },
              ],
            };

          case 'twig_switch_branch':
            return {
              content: [
                {
                  type: 'text',
                  text: JSON.stringify(await switchBranch(this.cli, args), null, 2),
                },
              ],
            };

          case 'twig_create_branch':
            return {
              content: [
                {
                  type: 'text',
                  text: JSON.stringify(await createBranch(this.cli, args), null, 2),
                },
              ],
            };

          case 'twig_delete_branch':
            return {
              content: [
                {
                  type: 'text',
                  text: JSON.stringify(await deleteBranch(this.cli, args), null, 2),
                },
              ],
            };

          case 'twig_add_dependency':
            return {
              content: [
                {
                  type: 'text',
                  text: JSON.stringify(await addDependency(this.cli, args), null, 2),
                },
              ],
            };

          case 'twig_remove_dependency':
            return {
              content: [
                {
                  type: 'text',
                  text: JSON.stringify(await removeDependency(this.cli, args), null, 2),
                },
              ],
            };

          case 'twig_add_root_branch':
            return {
              content: [
                {
                  type: 'text',
                  text: JSON.stringify(await addRootBranch(this.cli, args), null, 2),
                },
              ],
            };

          case 'twig_remove_root_branch':
            return {
              content: [
                {
                  type: 'text',
                  text: JSON.stringify(await removeRootBranch(this.cli, args), null, 2),
                },
              ],
            };

          case 'twig_list_root_branches':
            return {
              content: [
                {
                  type: 'text',
                  text: JSON.stringify(await listRootBranches(this.cli), null, 2),
                },
              ],
            };

          case 'twig_tidy_clean':
            return {
              content: [
                {
                  type: 'text',
                  text: JSON.stringify(await tidyClean(this.cli, args), null, 2),
                },
              ],
            };

          case 'twig_tidy_prune':
            return {
              content: [
                {
                  type: 'text',
                  text: JSON.stringify(await tidyPrune(this.cli, args), null, 2),
                },
              ],
            };

          case 'twig_github_create_pr':
            return {
              content: [
                {
                  type: 'text',
                  text: JSON.stringify(await createPullRequest(this.cli, args), null, 2),
                },
              ],
            };

          default:
            throw new Error(`Unknown tool: ${name}`);
        }
      } catch (error) {
        const errorMessage = error instanceof Error ? error.message : String(error);
        return {
          content: [
            {
              type: 'text',
              text: JSON.stringify({ error: errorMessage }, null, 2),
            },
          ],
          isError: true,
        };
      }
    });
  }

  private getToolDefinitions(): Tool[] {
    return [
      {
        name: 'twig_get_tree',
        description: 'Get the current twig branch tree structure with dependencies and status',
        inputSchema: {
          type: 'object',
          properties: {
            includeStatus: {
              type: 'boolean',
              description: 'Include branch status information (up-to-date, ahead, behind)',
              default: true,
            },
            includeOrphaned: {
              type: 'boolean',
              description: 'Include orphaned branches in the result',
              default: true,
            },
          },
        },
      },
      {
        name: 'twig_switch_branch',
        description: 'Switch to a specific branch',
        inputSchema: {
          type: 'object',
          properties: {
            branchName: {
              type: 'string',
              description: 'Name of the branch to switch to',
            },
          },
          required: ['branchName'],
        },
      },
      {
        name: 'twig_create_branch',
        description: 'Create a new branch with optional parent dependency',
        inputSchema: {
          type: 'object',
          properties: {
            branchName: {
              type: 'string',
              description: 'Name of the new branch',
            },
            parentBranch: {
              type: 'string',
              description: 'Parent branch to base the new branch on',
            },
            switchToBranch: {
              type: 'boolean',
              description: 'Whether to switch to the new branch after creation',
              default: true,
            },
          },
          required: ['branchName'],
        },
      },
      {
        name: 'twig_delete_branch',
        description: 'Delete a branch and clean up twig configuration',
        inputSchema: {
          type: 'object',
          properties: {
            branchName: {
              type: 'string',
              description: 'Name of the branch to delete',
            },
            force: {
              type: 'boolean',
              description: 'Force delete even if branch has unmerged changes',
              default: false,
            },
          },
          required: ['branchName'],
        },
      },
      {
        name: 'twig_add_dependency',
        description: 'Add a dependency relationship between two branches',
        inputSchema: {
          type: 'object',
          properties: {
            childBranch: {
              type: 'string',
              description: 'Child branch that depends on the parent',
            },
            parentBranch: {
              type: 'string',
              description: 'Parent branch that the child depends on',
            },
          },
          required: ['childBranch', 'parentBranch'],
        },
      },
      {
        name: 'twig_remove_dependency',
        description: 'Remove a dependency relationship between two branches',
        inputSchema: {
          type: 'object',
          properties: {
            childBranch: {
              type: 'string',
              description: 'Child branch to remove dependency from',
            },
            parentBranch: {
              type: 'string',
              description: 'Parent branch to remove dependency to',
            },
          },
          required: ['childBranch', 'parentBranch'],
        },
      },
      {
        name: 'twig_add_root_branch',
        description: 'Add a branch as a root branch',
        inputSchema: {
          type: 'object',
          properties: {
            branchName: {
              type: 'string',
              description: 'Name of the branch to add as root',
            },
          },
          required: ['branchName'],
        },
      },
      {
        name: 'twig_remove_root_branch',
        description: 'Remove a branch from root branches',
        inputSchema: {
          type: 'object',
          properties: {
            branchName: {
              type: 'string',
              description: 'Name of the branch to remove from roots',
            },
          },
          required: ['branchName'],
        },
      },
      {
        name: 'twig_list_root_branches',
        description: 'List all root branches',
        inputSchema: {
          type: 'object',
          properties: {},
        },
      },
      {
        name: 'twig_tidy_clean',
        description: 'Clean up branches with no unique commits and no children',
        inputSchema: {
          type: 'object',
          properties: {
            dryRun: {
              type: 'boolean',
              description: 'Preview changes without actually making them',
              default: false,
            },
            force: {
              type: 'boolean',
              description: 'Skip confirmation prompts',
              default: false,
            },
            aggressive: {
              type: 'boolean',
              description: 'Enable aggressive cleanup with reparenting',
              default: false,
            },
          },
        },
      },
      {
        name: 'twig_tidy_prune',
        description: 'Remove references to deleted branches from twig configuration',
        inputSchema: {
          type: 'object',
          properties: {
            dryRun: {
              type: 'boolean',
              description: 'Preview changes without actually making them',
              default: false,
            },
            force: {
              type: 'boolean',
              description: 'Skip confirmation prompts',
              default: false,
            },
          },
        },
      },
      {
        name: 'twig_github_create_pr',
        description: 'Create a GitHub pull request for the current branch',
        inputSchema: {
          type: 'object',
          properties: {
            title: {
              type: 'string',
              description: 'Title for the pull request',
            },
            description: {
              type: 'string',
              description: 'Description for the pull request',
            },
            draft: {
              type: 'boolean',
              description: 'Create as draft pull request',
              default: false,
            },
          },
        },
      },
    ];
  }

  async start() {
    const transport = new StdioServerTransport();
    await this.server.connect(transport);
    console.error('Twig MCP Server started');
  }
}

// Start the server
const server = new TwigMCPServer();
server.start().catch((error) => {
  console.error('Server failed to start:', error);
  process.exit(1);
});