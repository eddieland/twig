import { z } from 'zod';
import { TwigCLI } from '../utils/cli.js';
import { TwigTreeParser } from '../utils/parser.js';
import { GitHubPRResult } from '../types/twig.js';

const CreatePRInputSchema = z.object({
  title: z.string().optional(),
  description: z.string().optional(),
  draft: z.boolean().optional().default(false),
});

export async function createPullRequest(cli: TwigCLI, input: unknown): Promise<GitHubPRResult> {
  const { title, description, draft } = CreatePRInputSchema.parse(input);
  
  // Get current branch first
  const currentBranchResult = await cli.gitCurrentBranch();
  if (currentBranchResult.exitCode !== 0) {
    throw new Error('Failed to get current branch');
  }
  
  const currentBranch = currentBranchResult.stdout.trim();
  if (!currentBranch) {
    throw new Error('No current branch detected');
  }

  // Create the pull request
  const result = await cli.twigGitHubCreatePR();
  
  if (result.exitCode !== 0) {
    // Check for common GitHub errors
    if (result.stderr.includes('authentication') || result.stderr.includes('token')) {
      throw new Error('GitHub authentication required. Please set up your GitHub credentials.');
    } else if (result.stderr.includes('not found') || result.stderr.includes('repository')) {
      throw new Error('This repository may not be connected to GitHub or the remote is not accessible.');
    } else {
      throw new Error(`Failed to create pull request: ${result.stderr}`);
    }
  }

  const { url, title: prTitle } = TwigTreeParser.parseGitHubPRResult(result.stdout);
  
  if (!url) {
    throw new Error('Failed to parse pull request URL from output');
  }

  // Extract PR number from URL
  const prNumberMatch = url.match(/\/pull\/(\d+)$/);
  const prNumber = prNumberMatch ? parseInt(prNumberMatch[1], 10) : 0;

  return {
    url,
    number: prNumber,
    title: prTitle || title || `Pull request for ${currentBranch}`,
    branch: currentBranch,
  };
}