const fs = require('fs').promises;
const path = require('path');
const os = require('os');

/**
 * StateManager class for managing state file operations for coordination between containers.
 */
class StateManager {
  /**
   * Create a new StateManager instance.
   * @param {string} statePath - Path to the state directory. Defaults to process.env.STATE_PATH or '/workspace/.state'
   */
  constructor(statePath = process.env.STATE_PATH || '/workspace/.state') {
    this.statePath = statePath;
  }

  /**
   * Get the full path to a state file for a specific task.
   * @param {string} taskName - Name of the task
   * @returns {string} Full path to the state file
   */
  getStateFilePath(taskName) {
    return path.join(this.statePath, `${taskName}.state.json`);
  }

  /**
   * Get the full path to a lock file for a specific task.
   * @param {string} taskName - Name of the task
   * @returns {string} Full path to the lock file
   */
  getLockFilePath(taskName) {
    return path.join(this.statePath, `${taskName}.lock`);
  }

  /**
   * Create an initial state object with default values.
   * @returns {Object} Initial state object
   */
  createInitialState() {
    return {
      lastRun: null,
      lastSuccess: null,
      lastFailure: null,
      errorCount: 0,
      consecutiveFailures: 0,
      status: 'idle',
      lastTerminationReason: null,
      earlyTerminationCount: 0,
      totalExecutionTimeMs: 0,
      averageExecutionTimeMs: 0,
      successfulTerminations: 0,
      failedTerminations: 0
    };
  }

  /**
   * Ensure the state directory exists.
   * @private
   * @returns {Promise<void>}
   */
  async _ensureDirectory() {
    try {
      await fs.mkdir(this.statePath, { recursive: true });
    } catch (error) {
      if (error.code !== 'EEXIST') {
        throw new Error(`Failed to create state directory: ${error.message}`);
      }
    }
  }

  /**
   * Read state for a specific task.
   * @param {string} taskName - Name of the task
   * @returns {Promise<Object|null>} The state object or null if not found
   */
  async readState(taskName) {
    try {
      await this._ensureDirectory();
      const stateFilePath = this.getStateFilePath(taskName);
      const content = await fs.readFile(stateFilePath, 'utf8');
      const state = JSON.parse(content);
      // Ensure backward compatibility by merging with default values
      return { ...this.createInitialState(), ...state };
    } catch (error) {
      if (error.code === 'ENOENT') {
        return null;
      }
      throw new Error(`Failed to read state for task '${taskName}': ${error.message}`);
    }
  }

  /**
   * Write state for a specific task.
   * @param {string} taskName - Name of the task
   * @param {Object} state - State object to write
   * @returns {Promise<void>}
   */
  async writeState(taskName, state) {
    try {
      await this._ensureDirectory();
      const stateFilePath = this.getStateFilePath(taskName);
      const content = JSON.stringify(state, null, 2);
      await fs.writeFile(stateFilePath, content, 'utf8');
    } catch (error) {
      throw new Error(`Failed to write state for task '${taskName}': ${error.message}`);
    }
  }

  /**
   * Partially update state for a specific task.
   * @param {string} taskName - Name of the task
   * @param {Object} updates - Partial state updates to apply
   * @returns {Promise<Object>} The updated state object
   */
  async updateState(taskName, updates) {
    try {
      const currentState = await this.readState(taskName);
      const newState = currentState ? { ...currentState, ...updates } : { ...this.createInitialState(), ...updates };
      await this.writeState(taskName, newState);
      return newState;
    } catch (error) {
      throw new Error(`Failed to update state for task '${taskName}': ${error.message}`);
    }
  }

  /**
   * Reset state for a specific task to initial values.
   * @param {string} taskName - Name of the task
   * @returns {Promise<Object>} The reset state object
   */
  async resetState(taskName) {
    try {
      const initialState = this.createInitialState();
      await this.writeState(taskName, initialState);
      return initialState;
    } catch (error) {
      throw new Error(`Failed to reset state for task '${taskName}': ${error.message}`);
    }
  }

  /**
   * Record an early termination event.
   * @param {string} taskName - Name of the task
   * @param {string} reason - The reason for early termination
   * @param {number} executionTimeMs - Time before termination in milliseconds
   * @param {boolean} wasSuccessful - Whether the termination was successful
   * @returns {Promise<Object>} The updated state object
   */
  async recordEarlyTermination(taskName, reason, executionTimeMs, wasSuccessful) {
    try {
      const currentState = await this.readState(taskName);
      if (!currentState) {
        throw new Error(`No state found for task '${taskName}'`);
      }

      const newState = {
        ...currentState,
        lastTerminationReason: reason,
        earlyTerminationCount: currentState.earlyTerminationCount + 1,
        successfulTerminations: wasSuccessful ? currentState.successfulTerminations + 1 : currentState.successfulTerminations,
        failedTerminations: !wasSuccessful ? currentState.failedTerminations + 1 : currentState.failedTerminations,
        totalExecutionTimeMs: currentState.totalExecutionTimeMs + executionTimeMs
      };

      // Calculate average execution time (total executions = earlyTerminationCount + success runs)
      const totalExecutions = newState.earlyTerminationCount + (newState.lastSuccess ? 1 : 0);
      if (totalExecutions > 0) {
        newState.averageExecutionTimeMs = Math.round(newState.totalExecutionTimeMs / totalExecutions);
      }

      await this.writeState(taskName, newState);
      return newState;
    } catch (error) {
      throw new Error(`Failed to record early termination for task '${taskName}': ${error.message}`);
    }
  }

  /**
   * Record a successful completion.
   * @param {string} taskName - Name of the task
   * @param {number} executionTimeMs - Time taken for completion in milliseconds
   * @returns {Promise<Object>} The updated state object
   */
  async recordSuccess(taskName, executionTimeMs) {
    try {
      const currentState = await this.readState(taskName);
      if (!currentState) {
        throw new Error(`No state found for task '${taskName}'`);
      }

      const newState = {
        ...currentState,
        lastSuccess: new Date().toISOString(),
        status: 'success',
        consecutiveFailures: 0,
        totalExecutionTimeMs: currentState.totalExecutionTimeMs + executionTimeMs
      };

      // Calculate average execution time (total executions = earlyTerminationCount + success runs)
      const totalExecutions = newState.earlyTerminationCount + 1;
      if (totalExecutions > 0) {
        newState.averageExecutionTimeMs = Math.round(newState.totalExecutionTimeMs / totalExecutions);
      }

      await this.writeState(taskName, newState);
      return newState;
    } catch (error) {
      throw new Error(`Failed to record success for task '${taskName}': ${error.message}`);
    }
  }

  /**
   * Delete state file for a specific task.
   * @param {string} taskName - Name of the task
   * @returns {Promise<boolean>} True if deleted, false if didn't exist
   */
  async deleteState(taskName) {
    try {
      const stateFilePath = this.getStateFilePath(taskName);
      await fs.unlink(stateFilePath);
      return true;
    } catch (error) {
      if (error.code === 'ENOENT') {
        return false;
      }
      throw new Error(`Failed to delete state for task '${taskName}': ${error.message}`);
    }
  }

  /**
   * Acquire an exclusive lock for a task.
   * @param {string} taskName - Name of the task
   * @param {number} timeout - Timeout in milliseconds (default: 30000)
   * @returns {Promise<boolean>} True if lock acquired, false if timeout exceeded
   */
  async acquireLock(taskName, timeout = 30000) {
    const lockFilePath = this.getLockFilePath(taskName);
    const startTime = Date.now();
    const lockData = {
      pid: process.pid,
      timestamp: Date.now(),
      host: require('os').hostname()
    };

    while (Date.now() - startTime < timeout) {
      try {
        await this._ensureDirectory();
        const flags = fs.constants.O_CREAT | fs.constants.O_EXCL | fs.constants.O_WRONLY;
        const fileHandle = await fs.open(lockFilePath, flags);
        await fileHandle.writeFile(JSON.stringify(lockData, null, 2));
        await fileHandle.close();
        return true;
      } catch (error) {
        if (error.code === 'EEXIST') {
          // Lock file exists, wait a bit and retry
          await this._sleep(100);
        } else {
          throw new Error(`Failed to acquire lock for task '${taskName}': ${error.message}`);
        }
      }
    }

    throw new Error(`Failed to acquire lock for task '${taskName}': timeout after ${timeout}ms`);
  }

  /**
   * Release a lock for a task.
   * @param {string} taskName - Name of the task
   * @returns {Promise<boolean>} True if released, false if lock didn't exist
   */
  async releaseLock(taskName) {
    try {
      const lockFilePath = this.getLockFilePath(taskName);
      await fs.unlink(lockFilePath);
      return true;
    } catch (error) {
      if (error.code === 'ENOENT') {
        return false;
      }
      throw new Error(`Failed to release lock for task '${taskName}': ${error.message}`);
    }
  }

  /**
   * Check if a lock exists for a task.
   * @param {string} taskName - Name of the task
   * @returns {Promise<boolean>} True if locked, false otherwise
   */
  async isLocked(taskName) {
    try {
      const lockFilePath = this.getLockFilePath(taskName);
      await fs.access(lockFilePath, fs.constants.F_OK);
      return true;
    } catch (error) {
      if (error.code === 'ENOENT') {
        return false;
      }
      throw new Error(`Failed to check lock status for task '${taskName}': ${error.message}`);
    }
  }

  /**
   * Acquire a lock with retry logic.
   * @param {string} taskName - Name of the task
   * @param {Object} options - Retry options
   * @param {number} options.timeout - Timeout per attempt in milliseconds (default: 5000)
   * @param {number} options.maxRetries - Maximum number of retries (default: 3)
   * @param {number} options.retryDelay - Delay between retries in milliseconds (default: 1000)
   * @returns {Promise<boolean>} True if lock acquired
   */
  async acquireLockWithRetry(taskName, options = {}) {
    const {
      timeout = 5000,
      maxRetries = 3,
      retryDelay = 1000
    } = options;

    let lastError = null;

    for (let attempt = 0; attempt < maxRetries; attempt++) {
      try {
        return await this.acquireLock(taskName, timeout);
      } catch (error) {
        lastError = error;
        if (attempt < maxRetries - 1) {
          await this._sleep(retryDelay);
        }
      }
    }

    throw new Error(`Failed to acquire lock for task '${taskName}' after ${maxRetries} attempts: ${lastError.message}`);
  }

  /**
   * Cleanup stale locks that are either orphaned (process dead) or too old.
   * A lock is considered stale if:
   * - The process that created it is no longer running, OR
   * - The lock file is older than the specified max age
   * @param {number} maxAgeMs - Maximum age in milliseconds (optional, uses env var or default 5min)
   * @returns {Promise<Object>} Object with cleaned count and details of what was cleaned
   */
  async cleanupStaleLocks(maxAgeMs = null) {
    const effectiveMaxAge = maxAgeMs !== null ? maxAgeMs : this._getLockCleanupMaxAge();
    let cleanedCount = 0;
    const cleanedDetails = [];
    const currentHostname = os.hostname();
    const currentPid = process.pid;

    try {
      await this._ensureDirectory();
      const files = await fs.readdir(this.statePath);

      for (const file of files) {
        if (file.endsWith('.lock')) {
          const lockFilePath = path.join(this.statePath, file);
          const taskName = file.replace('.lock', '');

          try {
            const content = await fs.readFile(lockFilePath, 'utf8');
            let lockData = null;

            try {
              lockData = JSON.parse(content);
            } catch (parseError) {
              // Invalid lock file format, remove it
              console.warn(`[StateManager] Invalid lock file format for ${taskName}, removing...`);
              await fs.unlink(lockFilePath);
              cleanedCount++;
              cleanedDetails.push({ taskName, reason: 'invalid_format' });
              continue;
            }

            const lockPid = lockData.pid;
            const lockTimestamp = lockData.timestamp;
            const lockHostname = lockData.host;

            if (!lockPid) {
              // Lock file without PID, remove it
              console.warn(`[StateManager] Lock file for ${taskName} has no PID, removing...`);
              await fs.unlink(lockFilePath);
              cleanedCount++;
              cleanedDetails.push({ taskName, reason: 'no_pid' });
              continue;
            }

            // Check if process is running
            const isRunning = await this._isProcessRunning(lockPid);

            let shouldClean = false;
            let reason = '';

            if (!isRunning) {
              shouldClean = true;
              reason = 'process_dead';
              console.log(`[StateManager] Cleaning lock for ${taskName}: process ${lockPid} is not running`);
            } else {
              // Check lock age
              const age = Date.now() - lockTimestamp;
              if (age > effectiveMaxAge) {
                shouldClean = true;
                reason = 'too_old';
                const ageSeconds = Math.round(age / 1000);
                const maxAgeSeconds = Math.round(effectiveMaxAge / 1000);
                console.log(`[StateManager] Cleaning lock for ${taskName}: age ${ageSeconds}s exceeds threshold ${maxAgeSeconds}s`);
              } else if (lockHostname !== currentHostname) {
                // Lock from different host - could be stale if process died on that host
                // We can't verify processes on other hosts, so use a shorter timeout
                const crossHostAge = Date.now() - lockTimestamp;
                const crossHostThreshold = Math.min(effectiveMaxAge, 60000); // 1 minute max for cross-host locks
                if (crossHostAge > crossHostThreshold) {
                  shouldClean = true;
                  reason = 'cross_host_expired';
                  console.log(`[StateManager] Cleaning lock for ${taskName}: lock from different host expired`);
                }
              } else if (lockPid === currentPid) {
                // This is our own lock from a previous run that wasn't released
                // This shouldn't happen in normal operation but clean it up
                shouldClean = true;
                reason = 'same_pid_restarted';
                console.log(`[StateManager] Cleaning lock for ${taskName}: stale lock from current process (pid ${currentPid})`);
              }
            }

            if (shouldClean) {
              await fs.unlink(lockFilePath);
              cleanedCount++;
              cleanedDetails.push({
                taskName,
                pid: lockPid,
                hostname: lockHostname,
                reason
              });
            }

          } catch (error) {
            if (error.code === 'ENOENT') {
              // File was deleted by another process, skip
              continue;
            }
            console.error(`[StateManager] Error processing lock file ${file}: ${error.message}`);
            // Try to remove corrupted lock files
            try {
              await fs.unlink(lockFilePath);
              cleanedCount++;
              cleanedDetails.push({ taskName: file.replace('.lock', ''), reason: 'corrupted' });
            } catch (unlinkError) {
              // Failed to remove corrupted file, log and continue
              console.error(`[StateManager] Failed to remove corrupted lock file ${file}: ${unlinkError.message}`);
            }
          }
        }
      }

      if (cleanedCount > 0) {
        console.log(`[StateManager] Lock cleanup completed: ${cleanedCount} lock(s) cleaned`);
      } else {
        console.log(`[StateManager] Lock cleanup completed: no stale locks found`);
      }

    } catch (error) {
      if (error.code !== 'ENOENT') {
        throw new Error(`Failed to cleanup stale locks: ${error.message}`);
      }
    }

    return { cleanedCount, cleanedDetails };
  }

  /**
   * Remove all state files.
   * @returns {Promise<number>} Number of state files removed
   */
  async cleanupAllStates() {
    let removedCount = 0;

    try {
      await this._ensureDirectory();
      const files = await fs.readdir(this.statePath);

      for (const file of files) {
        if (file.endsWith('.state.json')) {
          const stateFilePath = path.join(this.statePath, file);
          try {
            await fs.unlink(stateFilePath);
            removedCount++;
          } catch (error) {
            // Skip files that can't be deleted
            continue;
          }
        }
      }
    } catch (error) {
      if (error.code !== 'ENOENT') {
        throw new Error(`Failed to cleanup all states: ${error.message}`);
      }
    }

    return removedCount;
  }

  /**
   * Sleep helper for delays.
   * @private
   * @param {number} ms - Milliseconds to sleep
   * @returns {Promise<void>}
   */
  _sleep(ms) {
    return new Promise(resolve => setTimeout(resolve, ms));
  }

  /**
   * Check if a process with the given PID is still running.
   * @private
   * @param {number} pid - Process ID to check
   * @returns {Promise<boolean>} True if process is running, false otherwise
   */
  async _isProcessRunning(pid) {
    try {
      // On Linux/Unix, process.kill(pid, 0) checks if process exists without sending a signal
      // If process doesn't exist, it throws ESRCH (No such process)
      process.kill(pid, 0);
      return true;
    } catch (error) {
      if (error.code === 'ESRCH') {
        // Process doesn't exist
        return false;
      }
      // Other errors (e.g., EPERM means we don't have permission but process exists)
      if (error.code === 'EPERM') {
        return true;
      }
      // Log unexpected errors but don't fail the cleanup
      console.warn(`[StateManager] Unexpected error checking PID ${pid}: ${error.message}`);
      return false;
    }
  }

  /**
   * Get the max age for stale locks from environment variable or use default.
   * @private
   * @returns {number} Max age in milliseconds
   */
  _getLockCleanupMaxAge() {
    const envValue = process.env.LOCK_CLEANUP_MAX_AGE_MS;
    if (envValue) {
      const parsed = parseInt(envValue, 10);
      if (!isNaN(parsed) && parsed > 0) {
        return parsed;
      }
      console.warn(`[StateManager] Invalid LOCK_CLEANUP_MAX_AGE_MS value: ${envValue}, using default`);
    }
    // Default to 5 minutes
    return 300000;
  }
}

module.exports = StateManager;
