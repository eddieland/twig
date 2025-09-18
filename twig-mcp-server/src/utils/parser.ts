import { TwigCLI, ExecResult } from './cli.js';
import { TwigTree, TwigBranch, BranchStatus } from '../types/twig.js';

export class TwigTreeParser {
  static parseTreeOutput(output: string): TwigTree {
    const lines = output.split('\n');
    const branches: TwigBranch[] = [];
    const rootBranches: string[] = [];
    const orphanedBranches: string[] = [];
    let currentBranch: string | undefined;

    let inOrphanedSection = false;
    
    for (const line of lines) {
      const trimmed = line.trim();
      
      // Skip empty lines and info lines
      if (!trimmed || trimmed.startsWith('ℹ') || trimmed.startsWith('To ') || trimmed.startsWith('•')) {
        continue;
      }

      // Check for orphaned section
      if (trimmed.includes('📝 Orphaned branches')) {
        inOrphanedSection = true;
        continue;
      }

      if (inOrphanedSection) {
        // Parse orphaned branches
        const match = trimmed.match(/^• (.+)$/);
        if (match) {
          const branchName = match[1];
          orphanedBranches.push(branchName);
          branches.push({
            name: branchName,
            children: [],
            status: 'untracked' as BranchStatus,
            isCurrentBranch: false,
            isRootBranch: false,
            isOrphaned: true,
          });
        }
        continue;
      }

      // Parse tree structure
      const cleanLine = line.replace(/\x1B\[[0-9;]*m/g, ''); // Remove ANSI codes
      const treeChars = cleanLine.match(/^[└├│─\s]*/);
      const indent = treeChars ? treeChars[0].length : 0;
      
      // Extract branch name
      let branchName = cleanLine.replace(/^[└├│─\s]*/, '').trim();
      
      // Check if current branch
      const isCurrentBranch = branchName.includes('(current)');
      if (isCurrentBranch) {
        branchName = branchName.replace(/\s+\(current\)$/, '').trim();
        currentBranch = branchName;
      }
      
      // Remove status indicators
      branchName = branchName.replace(/\s+\[.*?\]$/, '').trim();
      
      if (branchName && branchName.length > 0) {
        // Determine if it's a root branch (indent === 0)
        const isRootBranch = indent === 0;
        if (isRootBranch) {
          rootBranches.push(branchName);
        }

        // Parse status
        let status: BranchStatus = 'up-to-date';
        const statusMatch = line.match(/\[(.*?)\]/);
        if (statusMatch) {
          const statusText = statusMatch[1];
          if (statusText.includes('ahead')) status = 'ahead';
          else if (statusText.includes('behind')) status = 'behind';
          else if (statusText.includes('diverged')) status = 'diverged';
        }

        branches.push({
          name: branchName,
          children: [],
          status,
          isCurrentBranch,
          isRootBranch,
          isOrphaned: false,
        });
      }
    }

    // Build parent-child relationships
    this.buildRelationships(branches);

    return {
      branches,
      rootBranches,
      orphanedBranches,
      currentBranch,
    };
  }

  private static buildRelationships(branches: TwigBranch[]): void {
    // This is a simplified version - in practice, you'd parse the tree structure
    // to determine parent-child relationships based on indentation
    // For now, we'll leave this as a placeholder since the actual logic
    // would require more sophisticated parsing of the tree structure
  }

  static parseRootBranches(output: string): string[] {
    const lines = output.split('\n');
    const rootBranches: string[] = [];

    let inRootSection = false;
    for (const line of lines) {
      const trimmed = line.trim();
      
      if (trimmed.includes('Root branches:')) {
        inRootSection = true;
        continue;
      }
      
      if (inRootSection && trimmed.length > 0) {
        // Parse root branch lines like "ℹ   master"
        const match = trimmed.match(/^ℹ\s+(.+)$/);
        if (match && match[1].trim().length > 0) {
          rootBranches.push(match[1].trim());
        }
      }
    }

    return rootBranches;
  }

  static parseCleanResult(output: string): {
    branchesDeleted: string[];
    branchesReparented: Array<{ branch: string; oldParent: string; newParent: string }>;
  } {
    const lines = output.split('\n');
    const branchesDeleted: string[] = [];
    const branchesReparented: Array<{ branch: string; oldParent: string; newParent: string }> = [];

    for (const line of lines) {
      const trimmed = line.trim();
      
      // Parse deleted branches: "Deleted branch: branch-name"
      const deletedMatch = trimmed.match(/^Deleted branch: (.+)$/);
      if (deletedMatch) {
        branchesDeleted.push(deletedMatch[1]);
        continue;
      }

      // Parse reparented branches: "Reparented branch-name from old-parent to new-parent"
      const reparentedMatch = trimmed.match(/^Reparented (.+) from (.+) to (.+)$/);
      if (reparentedMatch) {
        branchesReparented.push({
          branch: reparentedMatch[1],
          oldParent: reparentedMatch[2],
          newParent: reparentedMatch[3],
        });
      }
    }

    return { branchesDeleted, branchesReparented };
  }

  static parsePruneResult(output: string): {
    removedBranches: string[];
    totalRemoved: number;
  } {
    const lines = output.split('\n');
    const removedBranches: string[] = [];
    let totalRemoved = 0;

    for (const line of lines) {
      const trimmed = line.trim();
      
      // Parse summary line: "Prune complete: removed X stale references"
      const summaryMatch = trimmed.match(/removed (\d+) stale references/);
      if (summaryMatch) {
        totalRemoved = parseInt(summaryMatch[1], 10);
        continue;
      }

      // Parse individual removed branches (would need to be added to twig output)
      const removedMatch = trimmed.match(/^• (.+)$/);
      if (removedMatch) {
        removedBranches.push(removedMatch[1]);
      }
    }

    return { removedBranches, totalRemoved };
  }

  static parseGitHubPRResult(output: string): {
    url?: string;
    title?: string;
  } {
    // Parse GitHub PR creation output for URL and title
    const urlMatch = output.match(/https:\/\/github\.com\/[^\s]+\/pull\/\d+/);
    const url = urlMatch ? urlMatch[0] : undefined;

    // Extract title if available in output
    const titleMatch = output.match(/Created pull request: (.+)/);
    const title = titleMatch ? titleMatch[1] : undefined;

    return { url, title };
  }
}