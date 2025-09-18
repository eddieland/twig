import { z } from 'zod';
import { TwigCLI } from '../utils/cli.js';

const SwitchBranchInputSchema = z.object({
  branchName: z.string(),
});

const CreateBranchInputSchema = z.object({
  branchName: z.string(),
  parentBranch: z.string().optional(),
  switchToBranch: z.boolean().optional().default(true),
});

const DeleteBranchInputSchema = z.object({
  branchName: z.string(),
  force: z.boolean().optional().default(false),
});

export async function switchBranch(cli: TwigCLI, input: unknown): Promise<{ success: true; currentBranch: string }> {
  const { branchName } = SwitchBranchInputSchema.parse(input);
  
  const result = await cli.twigSwitch(branchName);
  
  if (result.exitCode !== 0) {
    throw new Error(`Failed to switch to branch "${branchName}": ${result.stderr}`);
  }

  return {
    success: true,
    currentBranch: branchName,
  };
}

export async function createBranch(cli: TwigCLI, input: unknown): Promise<{ success: true; branchName: string; parentBranch?: string }> {
  const { branchName, parentBranch, switchToBranch } = CreateBranchInputSchema.parse(input);
  
  let result;
  
  if (parentBranch) {
    result = await cli.twigSwitchWithParent(branchName, parentBranch);
  } else {
    // Create branch without parent and then switch if needed
    result = await cli.execute(`git checkout -b "${branchName}"`);
    if (result.exitCode === 0 && !switchToBranch) {
      // Switch back to previous branch if we don't want to stay on new branch
      const currentResult = await cli.gitCurrentBranch();
      if (currentResult.exitCode === 0) {
        await cli.execute(`git checkout -`);
      }
    }
  }
  
  if (result.exitCode !== 0) {
    throw new Error(`Failed to create branch "${branchName}": ${result.stderr}`);
  }

  return {
    success: true,
    branchName,
    parentBranch,
  };
}

export async function deleteBranch(cli: TwigCLI, input: unknown): Promise<{ success: true; deletedBranch: string }> {
  const { branchName, force } = DeleteBranchInputSchema.parse(input);
  
  // First delete from git
  const gitResult = await cli.gitBranchDelete(branchName, force);
  
  if (gitResult.exitCode !== 0) {
    throw new Error(`Failed to delete branch "${branchName}": ${gitResult.stderr}`);
  }

  // Then clean up twig configuration
  const pruneResult = await cli.twigTidyPrune({ force: true });
  // Ignore prune errors as they're not critical for branch deletion

  return {
    success: true,
    deletedBranch: branchName,
  };
}