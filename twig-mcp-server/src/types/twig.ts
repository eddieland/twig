import { z } from 'zod';

// Branch status types
export const BranchStatus = z.enum(['up-to-date', 'ahead', 'behind', 'diverged', 'untracked']);
export type BranchStatus = z.infer<typeof BranchStatus>;

// Individual branch information
export const TwigBranch = z.object({
  name: z.string(),
  parent: z.string().optional(),
  children: z.array(z.string()),
  status: BranchStatus,
  isCurrentBranch: z.boolean(),
  isRootBranch: z.boolean(),
  isOrphaned: z.boolean(),
  hasUniqueCommits: z.boolean().optional(),
});
export type TwigBranch = z.infer<typeof TwigBranch>;

// Complete tree structure
export const TwigTree = z.object({
  branches: z.array(TwigBranch),
  rootBranches: z.array(z.string()),
  orphanedBranches: z.array(z.string()),
  currentBranch: z.string().optional(),
});
export type TwigTree = z.infer<typeof TwigTree>;

// Root branch information
export const RootBranch = z.object({
  branch: z.string(),
  isDefault: z.boolean().optional(),
});
export type RootBranch = z.infer<typeof RootBranch>;

// Dependency relationship
export const Dependency = z.object({
  child: z.string(),
  parent: z.string(),
});
export type Dependency = z.infer<typeof Dependency>;

// Tidy operation results
export const TidyCleanResult = z.object({
  branchesDeleted: z.array(z.string()),
  branchesReparented: z.array(z.object({
    branch: z.string(),
    oldParent: z.string(),
    newParent: z.string(),
  })),
  totalDeleted: z.number(),
  totalReparented: z.number(),
});
export type TidyCleanResult = z.infer<typeof TidyCleanResult>;

export const TidyPruneResult = z.object({
  removedBranches: z.array(z.string()),
  removedDependencies: z.number(),
  removedRoots: z.number(),
  removedMetadata: z.number(),
  totalRemoved: z.number(),
});
export type TidyPruneResult = z.infer<typeof TidyPruneResult>;

// Error types
export const TwigError = z.object({
  code: z.enum([
    'BRANCH_NOT_FOUND',
    'NOT_IN_REPO',
    'DEPENDENCY_CYCLE',
    'BRANCH_ALREADY_EXISTS',
    'CURRENT_BRANCH_OPERATION',
    'CLI_EXECUTION_ERROR',
    'INVALID_BRANCH_NAME',
    'GITHUB_ERROR',
  ]),
  message: z.string(),
  suggestions: z.array(z.string()).optional(),
  details: z.record(z.unknown()).optional(),
});
export type TwigError = z.infer<typeof TwigError>;

// GitHub PR creation result
export const GitHubPRResult = z.object({
  url: z.string(),
  number: z.number(),
  title: z.string(),
  branch: z.string(),
});
export type GitHubPRResult = z.infer<typeof GitHubPRResult>;