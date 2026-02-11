/**
 * CLI Executor Module
 * Handles execution of Kilo Code CLI with output monitoring and early termination
 * 
 * @module cli-executor
 */

const { spawn } = require('child_process');

// Error detection patterns
const ERROR_PATTERNS = {
  MISTAKE_LIMIT: [
    // Direct text match - most reliable
    /Mistake Limit Reached/i,
    // With common decorations
    /[\s\|]*✖\s*Mistake Limit Reached/i,
    /[\s\|]*X\s*Mistake Limit Reached/i,
    /[\s\|]*\*\s*Mistake Limit Reached/i,
    // Box border variations
    /[\s┌│─└]*Mistake Limit Reached[\s└│─]*/i,
  ],
  // Multi-line pattern for enhanced detection
  MISTAKE_LIMIT_MULTILINE: /Mistake Limit Reached[\s\S]{0,500}This may indicate a failure in the model/i
};

const ERROR_DETECTION_CONFIG = {
  minOutputLength: 50,
  maxBufferLines: 1000,
  maxBufferSize: 10 * 1024 * 1024, // 10MB
};

/**
 * Output buffer class for accumulating and managing CLI output
 */
class OutputBuffer {
  constructor(maxSize = ERROR_DETECTION_CONFIG.maxBufferSize) {
    this.fullOutput = '';
    this.maxSize = maxSize;
  }

  /**
   * Add output chunk to buffer
   * @param {Buffer|string} chunk - Output chunk
   * @returns {boolean} True if buffer is full
   */
  add(chunk) {
    const text = chunk.toString();
    
    // Check if adding would exceed limit
    if (this.fullOutput.length + text.length > this.maxSize) {
      // Truncate to keep recent output
      const keepSize = this.maxSize - text.length;
      if (keepSize > 0) {
        this.fullOutput = this.fullOutput.slice(-keepSize) + text;
      } else {
        // Even the new chunk is too big, truncate it
        this.fullOutput = text.slice(-this.maxSize);
      }
      return true; // Buffer is full
    }
    
    this.fullOutput += text;
    return false; // Buffer not full
  }

  /**
   * Get full accumulated output
   * @returns {string}
   */
  getFullOutput() {
    return this.fullOutput;
  }

  /**
   * Get recent portion of output
   * @param {number} length - Number of characters to return
   * @returns {string}
   */
  getRecent(length = 1000) {
    if (this.fullOutput.length <= length) {
      return this.fullOutput;
    }
    return this.fullOutput.slice(-length);
  }

  /**
   * Get output size in bytes
   * @returns {number}
   */
  getSize() {
    return this.fullOutput.length;
  }

  /**
   * Check if buffer is full
   * @returns {boolean}
   */
  isFull() {
    return this.fullOutput.length >= this.maxSize;
  }

  /**
   * Clear buffer
   */
  clear() {
    this.fullOutput = '';
  }
}

/**
 * Process lifecycle states
 */
const ProcessState = {
  STARTING: 'starting',
  RUNNING: 'running',
  TERMINATING: 'terminating',
  COMPLETED: 'completed',
  EARLY_TERMINATED: 'early_terminated',
  FAILED: 'failed'
};

/**
 * CLI Executor class for managing CLI processes with output monitoring
 */
class CLIExecutor {
  /**
   * Create a new CLIExecutor instance
   * @param {Object} options - Configuration options
   * @param {number} options.outputBufferLimit - Maximum buffer size in bytes (default: 10MB)
   * @param {number} options.gracefulTimeout - Timeout for graceful termination in ms (default: 5000)
   * @param {number} options.forceTimeout - Timeout for forceful termination in ms (default: 2000)
   */
  constructor(options = {}) {
    this.outputBufferLimit = options.outputBufferLimit || ERROR_DETECTION_CONFIG.maxBufferSize;
    this.gracefulTimeout = options.gracefulTimeout || 5000;
    this.forceTimeout = options.forceTimeout || 2000;
  }

  /**
   * Execute a CLI command with output monitoring and early termination
   * 
   * @param {Object} options - Execution options
   * @param {string} options.command - Command to execute (default: 'kilocode')
   * @param {Array<string>} options.args - Command arguments
   * @param {string} options.input - Input to pipe to stdin
   * @param {string} options.workspace - Workspace directory path
   * @param {number} options.timeout - Timeout in milliseconds (default: 1800000 = 30 minutes)
   * @param {Function} options.onOutput - Callback for each output chunk (text, accumulatedOutput)
   * @param {Function} options.onErrorDetected - Callback when error pattern is detected (pattern)
   * @returns {Promise<Object>} Execution result object
   * 
   * @example
   * const executor = new CLIExecutor();
   * const result = await executor.execute({
   *   command: 'kilocode',
   *   args: ['--mode', 'orchestrator'],
   *   input: 'My prompt content',
   *   workspace: '/path/to/workspace',
   *   timeout: 300000,
   *   onOutput: (chunk, output) => console.log(chunk),
   *   onErrorDetected: (pattern) => console.log('Error detected:', pattern)
   * });
   */
  async execute(options) {
    const {
      command = 'kilocode',
      args = [],
      input = '',
      workspace = process.env.WORKSPACE_PATH || '/workspace',
      timeout = 1800000, // 30 minutes default
      onOutput = null,
      onErrorDetected = null
    } = options;

    const startTime = Date.now();
    let cliProcess = null;
    let processState = ProcessState.STARTING;
    const buffer = new OutputBuffer(this.outputBufferLimit);
    let isMonitoring = true;
    let timeoutHandle = null;
    let bufferFull = false;

    // Return value
    let result = {
      exitCode: null,
      stdout: '',
      stderr: '',
      terminatedEarly: false,
      terminationReason: null,
      executionTimeMs: 0,
      signal: null
    };

    try {
      // Build full args if not provided
      const fullArgs = args.length > 0 ? args : [
        '--mode', 'orchestrator',
        '--auto',
        '--timeout', Math.floor(timeout / 1000).toString(),
        '--workspace', workspace
      ];

      cliProcess = spawn(command, fullArgs, {
        stdio: ['pipe', 'pipe', 'pipe'],
        shell: true,
        cwd: workspace
      });

      processState = ProcessState.RUNNING;

      // Write input to stdin
      if (input) {
        cliProcess.stdin.write(input);
        cliProcess.stdin.end();
      }

      // Handle stdout
      cliProcess.stdout.on('data', (chunk) => {
        if (!isMonitoring) return;

        const chunkText = chunk.toString();
        result.stdout += chunkText;

        // Add to detection buffer
        bufferFull = buffer.add(chunk);

        // Notify callback if provided
        if (onOutput) {
          try {
            onOutput(chunkText, 'stdout', buffer.getFullOutput());
          } catch (err) {
            // Ignore callback errors
          }
        }

        // Check for pattern if we have enough output
        if (!bufferFull && buffer.getFullOutput().length >= ERROR_DETECTION_CONFIG.minOutputLength) {
          const detectedPattern = this._detectPattern(buffer.getFullOutput());
          if (detectedPattern) {
            isMonitoring = false;
            processState = ProcessState.TERMINATING;
            
            // Notify callback if provided
            if (onErrorDetected) {
              try {
                onErrorDetected(detectedPattern);
              } catch (err) {
                // Ignore callback errors
              }
            }
            
            // Terminate process early
            this._terminateProcess(cliProcess, 'mistake_limit_reached').then(() => {
              clearTimeout(timeoutHandle);
            }).catch(() => {
              // Ignore termination errors
            });

            // Set result for early termination
            result.terminatedEarly = true;
            result.terminationReason = 'mistake_limit_reached';
            result.executionTimeMs = Date.now() - startTime;
          }
        }
      });

      // Handle stderr
      cliProcess.stderr.on('data', (chunk) => {
        if (!isMonitoring) return;

        const chunkText = chunk.toString();
        result.stderr += chunkText;

        // Add to detection buffer
        bufferFull = buffer.add(chunk);

        // Notify callback if provided
        if (onOutput) {
          try {
            onOutput(chunkText, 'stderr', buffer.getFullOutput());
          } catch (err) {
            // Ignore callback errors
          }
        }

        // Check for pattern if we have enough output
        if (!bufferFull && buffer.getFullOutput().length >= ERROR_DETECTION_CONFIG.minOutputLength) {
          const detectedPattern = this._detectPattern(buffer.getFullOutput());
          if (detectedPattern) {
            isMonitoring = false;
            processState = ProcessState.TERMINATING;
            
            // Notify callback if provided
            if (onErrorDetected) {
              try {
                onErrorDetected(detectedPattern);
              } catch (err) {
                // Ignore callback errors
              }
            }
            
            // Terminate process early
            this._terminateProcess(cliProcess, 'mistake_limit_reached').then(() => {
              clearTimeout(timeoutHandle);
            }).catch(() => {
              // Ignore termination errors
            });

            // Set result for early termination
            result.terminatedEarly = true;
            result.terminationReason = 'mistake_limit_reached';
            result.executionTimeMs = Date.now() - startTime;
          }
        }
      });

      // Handle process exit
      cliProcess.on('exit', (code, signal) => {
        if (timeoutHandle) {
          clearTimeout(timeoutHandle);
        }

        if (processState === ProcessState.RUNNING || processState === ProcessState.STARTING) {
          processState = code === 0 ? ProcessState.COMPLETED : ProcessState.FAILED;
        }

        result.exitCode = code;
        result.signal = signal;
        result.executionTimeMs = Date.now() - startTime;

        if (!result.terminationReason) {
          result.terminationReason = signal ? `signal_${signal}` : (code === 0 ? 'normal' : `exit_code_${code}`);
        }
      });

      // Handle process errors
      cliProcess.on('error', (error) => {
        if (timeoutHandle) {
          clearTimeout(timeoutHandle);
        }

        processState = ProcessState.FAILED;
        isMonitoring = false;
        
        result.exitCode = -1;
        result.executionTimeMs = Date.now() - startTime;
        result.terminationReason = 'process_error';
        result.error = error.message;
      });

      // Set up hard timeout
      timeoutHandle = setTimeout(() => {
        if (isMonitoring && cliProcess && !cliProcess.killed) {
          isMonitoring = false;
          processState = ProcessState.TERMINATING;
          
          this._terminateProcess(cliProcess, 'timeout').then(() => {
            result.terminatedEarly = true;
            result.terminationReason = 'timeout';
            result.executionTimeMs = Date.now() - startTime;
          }).catch(() => {
            // Ignore termination errors
            result.terminatedEarly = true;
            result.terminationReason = 'timeout';
            result.executionTimeMs = Date.now() - startTime;
          });
        }
      }, timeout);

      // Wait for process to complete
      await new Promise((resolve) => {
        const checkDone = () => {
          if (processState === ProcessState.COMPLETED || 
              processState === ProcessState.FAILED ||
              (processState === ProcessState.TERMINATING && result.terminatedEarly)) {
            resolve();
          } else if (cliProcess.killed || cliProcess.exitCode !== null) {
            resolve();
          }
        };

        cliProcess.on('exit', checkDone);
        cliProcess.on('error', checkDone);
        
        // Also check periodically for early termination
        const checkInterval = setInterval(() => {
          if (result.terminatedEarly || !isMonitoring) {
            clearInterval(checkInterval);
            resolve();
          }
        }, 100);
      });

      return result;

    } catch (error) {
      if (timeoutHandle) {
        clearTimeout(timeoutHandle);
      }

      processState = ProcessState.FAILED;
      
      return {
        exitCode: -1,
        stdout: result.stdout,
        stderr: result.stderr,
        terminatedEarly: false,
        terminationReason: 'spawn_error',
        executionTimeMs: Date.now() - startTime,
        error: error.message
      };
    }
  }

  /**
   * Detect if output contains error patterns
   * 
   * @param {string} output - Output to check
   * @returns {RegExp|null} The matched pattern, or null if no match
   * @private
   */
  _detectPattern(output) {
    if (!output || typeof output !== 'string') {
      return null;
    }

    // Check primary patterns (most efficient first)
    for (const pattern of ERROR_PATTERNS.MISTAKE_LIMIT) {
      if (pattern.test(output)) {
        return pattern;
      }
    }

    // Fallback to multi-line pattern if needed
    if (ERROR_PATTERNS.MISTAKE_LIMIT_MULTILINE.test(output)) {
      return ERROR_PATTERNS.MISTAKE_LIMIT_MULTILINE;
    }

    return null;
  }

  /**
   * Terminate a process with graceful then forceful approach
   * 
   * @param {ChildProcess} process - Process to terminate
   * @param {string} reason - Reason for termination
   * @returns {Promise<Object>} Termination result
   * @private
   */
  async _terminateProcess(process, reason) {
    const startTime = Date.now();
    let terminated = false;
    let method = null;
    const processState = {
      reason,
      gracefulTimeout: this.gracefulTimeout,
      forceTimeout: this.forceTimeout
    };

    try {
      // Phase 1: Graceful termination with SIGTERM
      if (process.pid && !process.killed) {
        process.kill('SIGTERM');
        method = 'graceful';

        // Wait for graceful shutdown
        await Promise.race([
          new Promise(resolve => {
            process.once('exit', resolve);
          }),
          new Promise(resolve => setTimeout(resolve, this.gracefulTimeout))
        ]);

        if (process.killed || process.exitCode !== null) {
          terminated = true;
        }
      }

      // Phase 2: Forceful termination with SIGKILL if graceful failed
      if (!terminated && process.pid && !process.killed) {
        process.kill('SIGKILL');
        method = 'force';

        await new Promise(resolve => setTimeout(resolve, this.forceTimeout));

        if (process.killed || process.exitCode !== null) {
          terminated = true;
        }
      }

      return {
        success: terminated,
        method: method,
        duration: Date.now() - startTime,
        exitCode: process.exitCode,
        signal: process.signalCode,
        ...processState
      };

    } catch (error) {
      return {
        success: false,
        method: method || 'failed',
        duration: Date.now() - startTime,
        error: error.message,
        ...processState
      };
    }
  }
}

// Export the class and utility functions
// Main export for direct require: const CLIExecutor = require('./lib/cli-executor');
module.exports = CLIExecutor;

// Additional exports for destructuring: const { CLIExecutor, OutputBuffer, ... } = require('./lib/cli-executor');
module.exports.CLIExecutor = CLIExecutor;
module.exports.OutputBuffer = OutputBuffer;
module.exports.ProcessState = ProcessState;
module.exports.ERROR_PATTERNS = ERROR_PATTERNS;
module.exports.ERROR_DETECTION_CONFIG = ERROR_DETECTION_CONFIG;
