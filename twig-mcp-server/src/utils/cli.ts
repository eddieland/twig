import { exec } from 'child_process';
import { promisify } from 'util';

const execAsync = promisify(exec);

export interface ExecResult {
  stdout: string;
  stderr: string;
  exitCode: number;
}

export class TwigCLI {
  private cwd: string;

  constructor(cwd: string = process.cwd()) {
    this.cwd = cwd;
  }

  async execute(command: string): Promise<ExecResult> {
    try {
      const { stdout, stderr } = await execAsync(command, { 
        cwd: this.cwd,
        timeout: 30000, // 30 second timeout
      });
      return {
        stdout: stdout.trim(),
        stderr: stderr.trim(),
        exitCode: 0,
      };
    } catch (error: any) {
      return {
        stdout: error.stdout?.trim() || '',
        stderr: error.stderr?.trim() || error.message || 'Unknown error',
        exitCode: error.code || 1,
      };
    }
  }

  async twigTree(): Promise<ExecResult> {
    return this.execute('twig tree');
  }

  async twigSwitch(branchName: string): Promise<ExecResult> {
    return this.execute(`twig switch "${branchName}"`);
  }

  async twigSwitchWithParent(branchName: string, parentBranch: string): Promise<ExecResult> {
    return this.execute(`twig switch -p "${parentBranch}" "${branchName}"`);
  }

  async twigBranchDepend(childBranch: string, parentBranch: string): Promise<ExecResult> {
    return this.execute(`twig branch depend "${childBranch}" "${parentBranch}"`);
  }

  async twigBranchRemoveDep(childBranch: string, parentBranch: string): Promise<ExecResult> {
    return this.execute(`twig branch remove-dep "${childBranch}" "${parentBranch}"`);
  }

  async twigBranchRootAdd(branchName: string): Promise<ExecResult> {
    return this.execute(`twig branch root add "${branchName}"`);
  }

  async twigBranchRootRemove(branchName: string): Promise<ExecResult> {
    return this.execute(`twig branch root remove "${branchName}"`);
  }

  async twigBranchRootList(): Promise<ExecResult> {
    return this.execute('twig branch root list');
  }

  async twigTidyClean(options: { dryRun?: boolean; force?: boolean; aggressive?: boolean } = {}): Promise<ExecResult> {
    const flags = [];
    if (options.dryRun) flags.push('--dry-run');
    if (options.force) flags.push('--force');
    if (options.aggressive) flags.push('--aggressive');
    
    return this.execute(`twig tidy clean ${flags.join(' ')}`);
  }

  async twigTidyPrune(options: { dryRun?: boolean; force?: boolean } = {}): Promise<ExecResult> {
    const flags = [];
    if (options.dryRun) flags.push('--dry-run');
    if (options.force) flags.push('--force');
    
    return this.execute(`twig tidy prune ${flags.join(' ')}`);
  }

  async twigGitHubCreatePR(): Promise<ExecResult> {
    return this.execute('twig github pr create-pr');
  }

  async gitBranch(options: string = ''): Promise<ExecResult> {
    return this.execute(`git branch ${options}`);
  }

  async gitBranchDelete(branchName: string, force: boolean = false): Promise<ExecResult> {
    const flag = force ? '-D' : '-d';
    return this.execute(`git branch ${flag} "${branchName}"`);
  }

  async gitCurrentBranch(): Promise<ExecResult> {
    return this.execute('git branch --show-current');
  }
}