import { z } from 'zod';
import { TwigCLI } from '../utils/cli.js';
import { TwigTreeParser } from '../utils/parser.js';
import { TidyCleanResult, TidyPruneResult } from '../types/twig.js';

const TidyCleanInputSchema = z.object({
  dryRun: z.boolean().optional().default(false),
  force: z.boolean().optional().default(false),
  aggressive: z.boolean().optional().default(false),
});

const TidyPruneInputSchema = z.object({
  dryRun: z.boolean().optional().default(false),
  force: z.boolean().optional().default(false),
});

export async function tidyClean(cli: TwigCLI, input: unknown): Promise<TidyCleanResult> {
  const { dryRun, force, aggressive } = TidyCleanInputSchema.parse(input);
  
  const result = await cli.twigTidyClean({ dryRun, force, aggressive });
  
  if (result.exitCode !== 0) {
    throw new Error(`Failed to run tidy clean: ${result.stderr}`);
  }

  const { branchesDeleted, branchesReparented } = TwigTreeParser.parseCleanResult(result.stdout);

  return {
    branchesDeleted,
    branchesReparented,
    totalDeleted: branchesDeleted.length,
    totalReparented: branchesReparented.length,
  };
}

export async function tidyPrune(cli: TwigCLI, input: unknown): Promise<TidyPruneResult> {
  const { dryRun, force } = TidyPruneInputSchema.parse(input);
  
  const result = await cli.twigTidyPrune({ dryRun, force });
  
  if (result.exitCode !== 0) {
    throw new Error(`Failed to run tidy prune: ${result.stderr}`);
  }

  const { removedBranches, totalRemoved } = TwigTreeParser.parsePruneResult(result.stdout);

  return {
    removedBranches,
    removedDependencies: 0, // Would need to be parsed from output
    removedRoots: 0,        // Would need to be parsed from output
    removedMetadata: 0,     // Would need to be parsed from output
    totalRemoved,
  };
}