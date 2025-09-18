#!/usr/bin/env node

/**
 * Simple test script to demonstrate Twig MCP Server capabilities
 * Run from a git repository with twig configuration
 */

import { spawn } from 'child_process';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

class MCPTester {
  constructor() {
    this.serverPath = join(__dirname, 'dist', 'index.js');
  }

  async callTool(method, params = {}) {
    return new Promise((resolve, reject) => {
      const server = spawn('node', [this.serverPath], {
        stdio: ['pipe', 'pipe', 'pipe'],
        cwd: process.cwd()
      });

      let stdout = '';
      let stderr = '';

      server.stdout.on('data', (data) => {
        stdout += data.toString();
      });

      server.stderr.on('data', (data) => {
        stderr += data.toString();
      });

      server.on('close', (code) => {
        if (code !== 0) {
          reject(new Error(`Server exited with code ${code}: ${stderr}`));
          return;
        }

        try {
          // Parse the JSON response (skip stderr which contains "Twig MCP Server started")
          const lines = stdout.split('\n').filter(line => line.trim());
          const jsonLine = lines.find(line => line.startsWith('{"result"') || line.startsWith('{"error"'));
          
          if (jsonLine) {
            const response = JSON.parse(jsonLine);
            resolve(response);
          } else {
            reject(new Error('No valid JSON response found'));
          }
        } catch (error) {
          reject(new Error(`Failed to parse JSON response: ${error.message}`));
        }
      });

      // Send the request
      const request = {
        jsonrpc: "2.0",
        id: Date.now(),
        method,
        params
      };

      server.stdin.write(JSON.stringify(request) + '\n');
      server.stdin.end();
    });
  }

  async testListTools() {
    console.log('🔧 Testing: List Available Tools');
    try {
      const response = await this.callTool('tools/list');
      const tools = response.result.tools;
      console.log(`✅ Found ${tools.length} tools:`);
      tools.forEach(tool => {
        console.log(`   - ${tool.name}: ${tool.description}`);
      });
      return true;
    } catch (error) {
      console.error(`❌ Error: ${error.message}`);
      return false;
    }
  }

  async testGetTree() {
    console.log('\n🌳 Testing: Get Twig Tree');
    try {
      const response = await this.callTool('tools/call', {
        name: 'twig_get_tree',
        arguments: { includeOrphaned: true }
      });
      
      if (response.result && response.result.content) {
        const content = JSON.parse(response.result.content[0].text);
        console.log(`✅ Tree retrieved successfully:`);
        console.log(`   - ${content.branches.length} branches found`);
        console.log(`   - ${content.rootBranches.length} root branches`);
        console.log(`   - Current branch: ${content.currentBranch || 'None'}`);
        
        if (content.branches.length > 0) {
          console.log(`   - Branch details:`);
          content.branches.slice(0, 3).forEach(branch => {
            const status = branch.isCurrentBranch ? ' (current)' : '';
            const root = branch.isRootBranch ? ' (root)' : '';
            console.log(`     • ${branch.name}${status}${root}`);
          });
        }
        return true;
      } else {
        console.error('❌ Unexpected response format');
        return false;
      }
    } catch (error) {
      console.error(`❌ Error: ${error.message}`);
      return false;
    }
  }

  async testListRootBranches() {
    console.log('\n🏠 Testing: List Root Branches');
    try {
      const response = await this.callTool('tools/call', {
        name: 'twig_list_root_branches',
        arguments: {}
      });
      
      if (response.result && response.result.content) {
        const content = JSON.parse(response.result.content[0].text);
        console.log(`✅ Root branches retrieved:`);
        if (content.rootBranches.length > 0) {
          content.rootBranches.forEach(branch => {
            console.log(`   - ${branch}`);
          });
        } else {
          console.log('   - No root branches found');
        }
        return true;
      } else {
        console.error('❌ Unexpected response format');
        return false;
      }
    } catch (error) {
      console.error(`❌ Error: ${error.message}`);
      return false;
    }
  }

  async runTests() {
    console.log('🚀 Twig MCP Server Test Suite\n');
    
    const tests = [
      () => this.testListTools(),
      () => this.testGetTree(),
      () => this.testListRootBranches(),
    ];

    let passed = 0;
    let total = tests.length;

    for (const test of tests) {
      if (await test()) {
        passed++;
      }
    }

    console.log(`\n📊 Test Results: ${passed}/${total} tests passed`);
    
    if (passed === total) {
      console.log('🎉 All tests passed! The Twig MCP Server is working correctly.');
    } else {
      console.log('⚠️  Some tests failed. Check the error messages above.');
    }

    return passed === total;
  }
}

// Run tests if this script is executed directly
if (import.meta.url === `file://${process.argv[1]}`) {
  const tester = new MCPTester();
  tester.runTests().then(success => {
    process.exit(success ? 0 : 1);
  }).catch(error => {
    console.error('Test suite failed:', error);
    process.exit(1);
  });
}

export { MCPTester };