import { z } from 'zod';
import { TwigCLI } from '../utils/cli.js';

const AddDependencyInputSchema = z.object({
  childBranch: z.string(),
  parentBranch: z.string(),
});

const RemoveDependencyInputSchema = z.object({
  childBranch: z.string(),
  parentBranch: z.string(),
});

export async function addDependency(cli: TwigCLI, input: unknown): Promise<{ success: true; childBranch: string; parentBranch: string }> {
  const { childBranch, parentBranch } = AddDependencyInputSchema.parse(input);
  
  const result = await cli.twigBranchDepend(childBranch, parentBranch);
  
  if (result.exitCode !== 0) {
    throw new Error(`Failed to add dependency "${childBranch}" -> "${parentBranch}": ${result.stderr}`);
  }

  return {
    success: true,
    childBranch,
    parentBranch,
  };
}

export async function removeDependency(cli: TwigCLI, input: unknown): Promise<{ success: true; childBranch: string; parentBranch: string }> {
  const { childBranch, parentBranch } = RemoveDependencyInputSchema.parse(input);
  
  const result = await cli.twigBranchRemoveDep(childBranch, parentBranch);
  
  if (result.exitCode !== 0) {
    throw new Error(`Failed to remove dependency "${childBranch}" -> "${parentBranch}": ${result.stderr}`);
  }

  return {
    success: true,
    childBranch,
    parentBranch,
  };
}