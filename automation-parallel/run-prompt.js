const { createScheduledTask, gracefulShutdown } = require('./lib/scheduler.js');
const WorkspaceManager = require('./lib/workspace-manager.js');
const StateManager = require('./lib/state-manager.js');
const CLIExecutor = require('./lib/cli-executor');
const fs = require('fs').promises;
const fsSync = require('fs');
const path = require('path');

/**
 * Logging utility with timestamp
 */
function logWithTimestamp(message) {
  const timestamp = new Date().toISOString();
  console.log(`[${timestamp}] ${message}`);
}

/**
 * Execute Kilo Code CLI with the given prompt content
 * @param {string} promptContent - The prompt content to pipe to the CLI
 * @param {string} promptName - Name of the prompt for logging purposes
 * @returns {Promise<Object>} Execution result with exitCode, terminatedEarly, terminationReason, executionTimeMs
 */
async function runKiloWithPrompt(promptContent, promptName) {
  logWithTimestamp(`[${TASK_NAME}] Starting ${promptName}...`);
  logWithTimestamp(`[${TASK_NAME}] Target Workspace: ${WORKSPACE_PATH}`);

  const executor = new CLIExecutor();

  logWithTimestamp(`[${TASK_NAME}] Running 'kilocode' with piped ${promptName} (${promptContent.length} chars)...`);

  const result = await executor.execute({
    command: 'kilocode',
    args: [
      '--mode', 'orchestrator',
      '--auto',
      '--timeout', '1800',  // 30 minutes (changed from 900 seconds)
      '--workspace', WORKSPACE_PATH
    ],
    input: promptContent,
    onOutput: (data) => {
      // Real-time output is inherited via shell: true in CLIExecutor
      console.log(data);
    },
    onErrorDetected: (pattern) => {
      logWithTimestamp(`[${TASK_NAME}] Error pattern detected: ${pattern}`);
    }
  });

  if (result.terminatedEarly) {
    logWithTimestamp(`[${TASK_NAME}] ${promptName} terminated early: ${result.terminationReason} (${result.executionTimeMs}ms)`);
  } else if (result.exitCode === 0) {
    logWithTimestamp(`[${TASK_NAME}] ${promptName} completed successfully (${result.executionTimeMs}ms)`);
  } else {
    logWithTimestamp(`[${TASK_NAME}] ${promptName} completed with exit code ${result.exitCode} (${result.executionTimeMs}ms)`);
  }

  return result;
}

/**
 * Parse command line arguments
 * @returns {Object} Parsed arguments with defaults
 */
function parseArgs() {
  const args = {
    workspace: '/workspace',
    state: '/workspace/.state',
    interval: 0, // No delay - continuous loop for main dev loop
    timeout: 900000, // 15 minutes
    immediate: true
  };

  for (let i = 2; i < process.argv.length; i++) {
    if (process.argv[i] === '--workspace' && i + 1 < process.argv.length) {
      args.workspace = process.argv[++i];
    } else if (process.argv[i] === '--state' && i + 1 < process.argv.length) {
      args.state = process.argv[++i];
    } else if (process.argv[i] === '--interval' && i + 1 < process.argv.length) {
      args.interval = parseInt(process.argv[++i], 10);
      if (isNaN(args.interval) || args.interval < 0) {
        console.error('Error: --interval must be a non-negative integer');
        showUsage();
        process.exit(1);
      }
    } else if (process.argv[i] === '--timeout' && i + 1 < process.argv.length) {
      args.timeout = parseInt(process.argv[++i], 10);
      if (isNaN(args.timeout) || args.timeout <= 0) {
        console.error('Error: --timeout must be a positive integer');
        showUsage();
        process.exit(1);
      }
    } else if (process.argv[i] === '--immediate') {
      args.immediate = true;
    } else if (process.argv[i] === '--no-immediate') {
      args.immediate = false;
    } else if (process.argv[i] === '--help' || process.argv[i] === '-h') {
      showUsage();
      process.exit(0);
    } else if (process.argv[i].startsWith('--')) {
      console.error(`Error: Unknown argument '${process.argv[i]}'`);
      showUsage();
      process.exit(1);
    }
  }

  return args;
}

/**
 * Display usage information
 */
function showUsage() {
  console.log(`
Usage: node run-prompt.js [options]

Options:
  --workspace <path>     Workspace directory (default: /workspace)
  --state <path>          State directory (default: /workspace/.state)
  --interval <ms>         Schedule interval in milliseconds (default: 0 - no delay)
  --timeout <ms>          Timeout in milliseconds for task execution (default: 900000)
  --immediate             Run immediately on startup (default: true)
  --no-immediate          Don't run immediately on startup
  --help, -h              Show this help message

Examples:
  node run-prompt.js
  node run-prompt.js --workspace /my/workspace --interval 600000
  node run-prompt.js --state /my/state --no-immediate
  node run-prompt.js --timeout 600000
`);
}

// Set up environment
const TASK_NAME = 'prompt';
const DEFAULT_INTERVAL_MS = 0; // No delay - continuous loop for main dev loop
const args = parseArgs();
const WORKSPACE_PATH = args.workspace;
const STATE_PATH = args.state;
const SCHEDULE_INTERVAL_MS = args.interval;
const TASK_TIMEOUT_MS = args.timeout;
const SCHEDULE_IMMEDIATE = args.immediate;

// Set environment variables for scheduler
process.env.TASK_NAME = TASK_NAME;
process.env.SCHEDULE_INTERVAL_MS = SCHEDULE_INTERVAL_MS.toString();
process.env.TASK_TIMEOUT_MS = TASK_TIMEOUT_MS.toString();
process.env.SCHEDULE_IMMEDIATE = SCHEDULE_IMMEDIATE;

// Initialize managers
const workspaceManager = new WorkspaceManager(WORKSPACE_PATH);
const stateManager = new StateManager(STATE_PATH);

/**
 * Perform startup lock cleanup to remove stale locks from previous runs
 * This prevents containers from failing to start after crashes
 */
async function performStartupCleanup() {
  logWithTimestamp(`[${TASK_NAME}] Performing startup lock cleanup...`);
  try {
    const result = await stateManager.cleanupStaleLocks();
    if (result.cleanedCount > 0) {
      logWithTimestamp(`[${TASK_NAME}] Cleaned up ${result.cleanedCount} stale lock(s)`);
      for (const detail of result.cleanedDetails) {
        logWithTimestamp(`[${TASK_NAME}]   - ${detail.taskName}: ${detail.reason} (pid: ${detail.pid}, host: ${detail.hostname})`);
      }
    } else {
      logWithTimestamp(`[${TASK_NAME}] No stale locks found`);
    }
  } catch (error) {
    logWithTimestamp(`[${TASK_NAME}] Warning: Failed to perform startup lock cleanup: ${error.message}`);
    // Don't fail startup if cleanup fails, just log and continue
  }
}

/**
 * Check if a markdown file contains work items
 * @param {string} content - Content of the markdown file
 * @returns {boolean} True if work items exist, false otherwise
 */
function hasWorkItems(content) {
  if (!content || content.trim().length === 0) {
    return false;
  }
  // Check for task markers like checkboxes or numbered lists
  const taskPatterns = [
    /^\s*-\s+\[[ x]\]\s+/m,  // Checkboxes
    /^\s*\d+\.\s+/m,         // Numbered lists
    /^\s*-\s+.+/m            // Bullet lists with content
  ];
  return taskPatterns.some(pattern => pattern.test(content));
}

/**
 * Extract work items from markdown content
 * @param {string} content - Content of the markdown file
 * @returns {Array} Array of work item strings
 */
function extractWorkItems(content) {
  if (!content || content.trim().length === 0) {
    return [];
  }
  
  const items = [];
  const lines = content.split('\n');
  
  for (const line of lines) {
    const trimmed = line.trim();
    // Match checkboxes, numbered items, or bullet items
    if (
      /^-\s+\[[ x]\]\s+/.test(trimmed) ||
      /^\d+\.\s+/.test(trimmed) ||
      /^-\s+/.test(trimmed)
    ) {
      items.push(trimmed);
    }
  }
  
  return items;
}

/**
 * Execute a prompt template with the given context
 * @param {string} promptTemplate - Path to the prompt template file
 * @param {Object} context - Context object with variables to substitute
 * @returns {Promise<Object>} Result object with success, output, and error
 */
async function executePromptTemplate(promptTemplate, context) {
  try {
    const templatePath = path.join(__dirname, 'prompts', 'development', promptTemplate);
    const templateContent = await fs.readFile(templatePath, 'utf-8');
    
    // Simple template substitution
    let processedContent = templateContent;
    for (const [key, value] of Object.entries(context)) {
      const placeholder = `{{${key}}}`;
      processedContent = processedContent.replaceAll(placeholder, value);
    }
    
    return {
      success: true,
      output: processedContent,
      error: null
    };
  } catch (error) {
    return {
      success: false,
      output: null,
      error: error.message
    };
  }
}

/**
 * Check for continuation - detect if a prompt run was interrupted
 * @returns {string|null} Returns the prompt name if a marker file exists, null otherwise
 */
function checkContinuation() {
  const markerPath = path.join(WORKSPACE_PATH, '.prompt_in_progress');
  if (fsSync.existsSync(markerPath)) {
    const stateFilePath = path.join(WORKSPACE_PATH, 'PROMPT_STATE.md');
    const hasStateFile = fsSync.existsSync(stateFilePath);
    logWithTimestamp(`[${TASK_NAME}] Found incomplete prompt session`);
    logWithTimestamp(`[${TASK_NAME}]   - Marker: .prompt_in_progress`);
    logWithTimestamp(`[${TASK_NAME}]   - State file exists: ${hasStateFile}`);
    if (hasStateFile) {
      try {
        const stateContent = fsSync.readFileSync(stateFilePath, 'utf8');
        // Extract status line if present
        const statusMatch = stateContent.match(/Status:\s*(\w+)/i);
        if (statusMatch) {
          logWithTimestamp(`[${TASK_NAME}]   - Status: ${statusMatch[1]}`);
        }
      } catch (e) {
        // Ignore read errors
      }
    }
    return 'prompt';
  }
  return null;
}

/**
 * Process work items and write results
 * @param {Array} workItems - Array of work item strings
 * @returns {Promise<void>}
 */
async function processWorkItems(workItems) {
  logWithTimestamp(`[${TASK_NAME}] Processing ${workItems.length} work items`);

  // Build context for prompt execution
  const context = {
    taskCount: workItems.length,
    tasks: workItems.join('\n'),
    timestamp: new Date().toISOString()
  };

  // Execute the PROMPT.md template
  const promptResult = await executePromptTemplate('PROMPT.md', context);

  if (!promptResult.success) {
    throw new Error(`Failed to execute PROMPT.md template: ${promptResult.error}`);
  }

  // Execute the Kilo Code CLI with the processed prompt
  logWithTimestamp(`[${TASK_NAME}] Executing Kilo Code CLI with PROMPT prompt...`);
  const result = await runKiloWithPrompt(promptResult.output, 'PROMPT');

  // Handle early termination
  if (result.terminatedEarly) {
    await stateManager.recordEarlyTermination(
      TASK_NAME,
      result.terminationReason,
      result.executionTimeMs,
      false  // terminated early means not successful
    );
    logWithTimestamp(`[${TASK_NAME}] Early termination recorded: ${result.terminationReason}`);
    throw new Error(`CLI terminated early: ${result.terminationReason}`);
  }

  // Handle successful completion
  if (result.exitCode === 0) {
    await stateManager.recordSuccess(TASK_NAME, result.executionTimeMs);
    logWithTimestamp(`[${TASK_NAME}] Success recorded (${result.executionTimeMs}ms)`);
  }

  // Check for .done file after successful execution
  if (result.exitCode === 0) {
    const doneFilePath = path.join(WORKSPACE_PATH, '.done');
    if (fsSync.existsSync(doneFilePath)) {
      logWithTimestamp(`[${TASK_NAME}] Done file detected, stopping...`);
      process.exit(0);
    }
  }

  // Handle exit code failure
  if (result.exitCode !== 0) {
    throw new Error(`CLI execution failed with exit code ${result.exitCode}`);
  }

  // Write results to a log file in the workspace for reference
  const resultLogPath = path.join(WORKSPACE_PATH, `.prompt-output-${Date.now()}.md`);
  await fs.writeFile(resultLogPath, promptResult.output, 'utf-8');

  logWithTimestamp(`[${TASK_NAME}] Results written to ${resultLogPath}`);
}

/**
 * Main prompt execution handler
 * This function is called by the scheduler on a recurring basis
 */
async function promptExecutionHandler() {
  const handlerTimestamp = new Date().toISOString();
  logWithTimestamp(`[${TASK_NAME}] Starting prompt execution handler at ${handlerTimestamp}`);
  
  let lockAcquired = false;
  
  try {
    // Acquire a lock with retry logic
    logWithTimestamp(`[${TASK_NAME}] Acquiring lock...`);
    await stateManager.acquireLockWithRetry(TASK_NAME, {
      timeout: 5000,
      maxRetries: 3,
      retryDelay: 1000
    });
    lockAcquired = true;
    logWithTimestamp(`[${TASK_NAME}] Lock acquired successfully`);
    
    // Update state with lastRun timestamp
    await stateManager.updateState(TASK_NAME, {
      lastRun: handlerTimestamp,
      status: 'running'
    });
    logWithTimestamp(`[${TASK_NAME}] State updated with lastRun timestamp`);
    
    // Read workspace files
    logWithTimestamp(`[${TASK_NAME}] Reading workspace files...`);
    const [todoContent, backlogContent, completedContent, blockersContent, prdContent] = await Promise.all([
      workspaceManager.readTodoFile().catch(() => ''),
      workspaceManager.readBacklogFile().catch(() => ''),
      workspaceManager.readCompletedFile().catch(() => ''),
      workspaceManager.readBlockersFile().catch(() => ''),
      workspaceManager.readPrdFile().catch(() => '')
    ]);
    
    logWithTimestamp(`[${TASK_NAME}] Workspace files read successfully`);
    logWithTimestamp(`[${TASK_NAME}] TODO.md length: ${todoContent.length} bytes`);
    logWithTimestamp(`[${TASK_NAME}] BACKLOG.md length: ${backlogContent.length} bytes`);
    
    // Check for new work items in TODO.md
    const hasWork = hasWorkItems(todoContent);
    logWithTimestamp(`[${TASK_NAME}] Work items detected in TODO.md: ${hasWork}`);
    
    if (hasWork) {
      const workItems = extractWorkItems(todoContent);
      logWithTimestamp(`[${TASK_NAME}] Found ${workItems.length} work items`);
      
      // Load and execute the PROMPT.md prompt template
      logWithTimestamp(`[${TASK_NAME}] Executing PROMPT.md template...`);
      await processWorkItems(workItems);
      
      // Update state with success
      await stateManager.updateState(TASK_NAME, {
        lastSuccess: new Date().toISOString(),
        status: 'success',
        consecutiveFailures: 0
      });
      logWithTimestamp(`[${TASK_NAME}] State updated with success`);
    } else {
      logWithTimestamp(`[${TASK_NAME}] No work items found, skipping prompt execution`);
      
      // Update state to indicate no work was done
      await stateManager.updateState(TASK_NAME, {
        status: 'idle'
      });
    }
    
    // Release the lock
    await stateManager.releaseLock(TASK_NAME);
    lockAcquired = false;
    logWithTimestamp(`[${TASK_NAME}] Lock released`);
    
    logWithTimestamp(`[${TASK_NAME}] Prompt execution handler completed successfully`);
    
  } catch (error) {
    logWithTimestamp(`[${TASK_NAME}] Error in prompt execution handler: ${error.message}`);
    
    // Update state with failure
    try {
      const currentState = await stateManager.readState(TASK_NAME) || {};
      const consecutiveFailures = (currentState.consecutiveFailures || 0) + 1;
      
      await stateManager.updateState(TASK_NAME, {
        lastFailure: new Date().toISOString(),
        status: 'failed',
        error: error.message,
        errorCount: (currentState.errorCount || 0) + 1,
        consecutiveFailures: consecutiveFailures
      });
      logWithTimestamp(`[${TASK_NAME}] State updated with failure (consecutive failures: ${consecutiveFailures})`);
    } catch (stateError) {
      logWithTimestamp(`[${TASK_NAME}] Failed to update error state: ${stateError.message}`);
    }
    
    // Ensure lock is released even if an error occurred
    if (lockAcquired) {
      try {
        await stateManager.releaseLock(TASK_NAME);
        logWithTimestamp(`[${TASK_NAME}] Lock released after error`);
      } catch (lockError) {
        logWithTimestamp(`[${TASK_NAME}] Failed to release lock: ${lockError.message}`);
      }
    }
    
    // Re-throw the error for the scheduler to handle with retry logic
    throw error;
  }
}

// Create the scheduled task
const task = createScheduledTask(promptExecutionHandler, {
  taskName: TASK_NAME,
  intervalMs: SCHEDULE_INTERVAL_MS,
  timeoutMs: TASK_TIMEOUT_MS,
  immediate: true
});

// Set up graceful shutdown handlers
process.on('SIGTERM', async () => {
  logWithTimestamp(`[${TASK_NAME}] Received SIGTERM signal, initiating graceful shutdown...`);
  try {
    await gracefulShutdown([task], 30000);
    logWithTimestamp(`[${TASK_NAME}] Graceful shutdown complete, exiting with code 0`);
    process.exit(0);
  } catch (error) {
    logWithTimestamp(`[${TASK_NAME}] Error during graceful shutdown: ${error.message}`);
    process.exit(1);
  }
});

process.on('SIGINT', async () => {
  logWithTimestamp(`[${TASK_NAME}] Received SIGINT signal, initiating graceful shutdown...`);
  try {
    await gracefulShutdown([task], 30000);
    logWithTimestamp(`[${TASK_NAME}] Graceful shutdown complete, exiting with code 0`);
    process.exit(0);
  } catch (error) {
    logWithTimestamp(`[${TASK_NAME}] Error during graceful shutdown: ${error.message}`);
    process.exit(1);
  }
});

// Handle uncaught exceptions
process.on('uncaughtException', (error) => {
  logWithTimestamp(`[${TASK_NAME}] Uncaught exception: ${error.message}`);
  logWithTimestamp(`[${TASK_NAME}] Stack trace: ${error.stack}`);
  process.exit(1);
});

// Handle unhandled promise rejections
process.on('unhandledRejection', (reason, promise) => {
  logWithTimestamp(`[${TASK_NAME}] Unhandled rejection at ${promise}: ${reason}`);
  process.exit(1);
});

// Start the task
logWithTimestamp(`[${TASK_NAME}] ================================================`);
logWithTimestamp(`[${TASK_NAME}] Starting Prompt Runner`);
logWithTimestamp(`[${TASK_NAME}] Workspace Path: ${WORKSPACE_PATH}`);
logWithTimestamp(`[${TASK_NAME}] State Path: ${STATE_PATH}`);
logWithTimestamp(`[${TASK_NAME}] Schedule Interval: ${SCHEDULE_INTERVAL_MS}ms`);
logWithTimestamp(`[${TASK_NAME}] Schedule Immediate: ${SCHEDULE_IMMEDIATE}`);
logWithTimestamp(`[${TASK_NAME}] ================================================`);

// Perform startup cleanup before starting the task
(async () => {
  await performStartupCleanup();

  task.start();

  logWithTimestamp(`[${TASK_NAME}] Task started successfully. Waiting for scheduled executions...`);
})();
