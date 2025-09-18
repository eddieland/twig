import { z } from 'zod';
import { TwigCLI } from '../utils/cli.js';
import { TwigTreeParser } from '../utils/parser.js';

const AddRootBranchInputSchema = z.object({
  branchName: z.string(),
});

const RemoveRootBranchInputSchema = z.object({
  branchName: z.string(),
});

export async function addRootBranch(cli: TwigCLI, input: unknown): Promise<{ success: true; rootBranch: string }> {
  const { branchName } = AddRootBranchInputSchema.parse(input);
  
  const result = await cli.twigBranchRootAdd(branchName);
  
  if (result.exitCode !== 0) {
    throw new Error(`Failed to add root branch "${branchName}": ${result.stderr}`);
  }

  return {
    success: true,
    rootBranch: branchName,
  };
}

export async function removeRootBranch(cli: TwigCLI, input: unknown): Promise<{ success: true; removedRootBranch: string }> {
  const { branchName } = RemoveRootBranchInputSchema.parse(input);
  
  const result = await cli.twigBranchRootRemove(branchName);
  
  if (result.exitCode !== 0) {
    throw new Error(`Failed to remove root branch "${branchName}": ${result.stderr}`);
  }

  return {
    success: true,
    removedRootBranch: branchName,
  };
}

export async function listRootBranches(cli: TwigCLI): Promise<{ rootBranches: string[] }> {
  const result = await cli.twigBranchRootList();
  
  if (result.exitCode !== 0) {
    throw new Error(`Failed to list root branches: ${result.stderr}`);
  }

  const rootBranches = TwigTreeParser.parseRootBranches(result.stdout);

  return {
    rootBranches,
  };
}