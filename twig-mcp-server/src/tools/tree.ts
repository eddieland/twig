import { z } from 'zod';
import { TwigCLI } from '../utils/cli.js';
import { TwigTreeParser } from '../utils/parser.js';
import { TwigTree } from '../types/twig.js';

const GetTreeInputSchema = z.object({
  includeStatus: z.boolean().optional().default(true),
  includeOrphaned: z.boolean().optional().default(true),
});

export async function getTree(cli: TwigCLI, input: unknown): Promise<TwigTree> {
  const params = GetTreeInputSchema.parse(input);
  
  const result = await cli.twigTree();
  
  if (result.exitCode !== 0) {
    throw new Error(`Failed to get twig tree: ${result.stderr}`);
  }

  const tree = TwigTreeParser.parseTreeOutput(result.stdout);
  
  // Filter orphaned branches if not requested
  if (!params.includeOrphaned) {
    tree.orphanedBranches = [];
    tree.branches = tree.branches.filter(b => !b.isOrphaned);
  }

  return tree;
}