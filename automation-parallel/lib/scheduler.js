const EventEmitter = require('events');

/**
 * Logging utility with timestamp
 */
function logWithTimestamp(message) {
  const timestamp = new Date().toISOString();
  console.log(`[${timestamp}] ${message}`);
}

/**
 * ScheduledTask class for managing recurring tasks
 */
class ScheduledTask {
  constructor(taskName, intervalMs, handlerFunction, options = {}) {
    this.taskName = taskName;
    this.intervalMs = intervalMs;
    this.handlerFunction = handlerFunction;
    this.options = {
      immediate: options.immediate !== undefined ? options.immediate : false,
      maxRetries: options.maxRetries !== undefined ? options.maxRetries : 3
    };
    
    this._timer = null;
    this._isRunning = false;
    this._lastRunTimestamp = null;
    this._successCount = 0;
    this._failureCount = 0;
    this._currentRetry = 0;
  }

  /**
   * Start scheduling the task
   */
  start() {
    if (this._isRunning) {
      logWithTimestamp(`[${this.taskName}] Task is already running`);
      return;
    }

    this._isRunning = true;
    logWithTimestamp(`[${this.taskName}] Starting scheduled task (interval: ${this.intervalMs}ms, immediate: ${this.options.immediate})`);

    if (this.options.immediate) {
      // Execute immediately on start
      this._executeHandler();
    }

    this._scheduleNextRun();
  }

  /**
   * Stop scheduling the task
   */
  stop() {
    if (!this._isRunning) {
      logWithTimestamp(`[${this.taskName}] Task is not running`);
      return;
    }

    this._isRunning = false;
    
    if (this._timer) {
      clearTimeout(this._timer);
      this._timer = null;
    }

    logWithTimestamp(`[${this.taskName}] Stopped scheduled task`);
  }

  /**
   * Schedule the next run of the task
   */
  _scheduleNextRun() {
    if (!this._isRunning) {
      return;
    }

    this._timer = setTimeout(() => {
      this._executeHandler();
    }, this.intervalMs);
  }

  /**
   * Execute the handler function with retry logic
   */
  async _executeHandler() {
    if (!this._isRunning) {
      return;
    }

    this._currentRetry = 0;
    await this._executeWithRetry();
  }

  /**
   * Execute handler with exponential backoff retry logic
   */
  async _executeWithRetry() {
    try {
      this._lastRunTimestamp = new Date().toISOString();
      logWithTimestamp(`[${this.taskName}] Executing handler (attempt ${this._currentRetry + 1}/${this.options.maxRetries + 1})`);
      
      await this.handlerFunction();
      
      // Success
      this._successCount++;
      this._currentRetry = 0;
      logWithTimestamp(`[${this.taskName}] Handler executed successfully`);
      
    } catch (error) {
      this._failureCount++;
      logWithTimestamp(`[${this.taskName}] Error executing handler: ${error.message}`);
      
      if (this._currentRetry < this.options.maxRetries) {
        // Retry with exponential backoff
        const backoffDelay = 100 * Math.pow(2, this._currentRetry);
        this._currentRetry++;
        
        logWithTimestamp(`[${this.taskName}] Retrying in ${backoffDelay}ms (attempt ${this._currentRetry + 1}/${this.options.maxRetries + 1})`);
        
        return new Promise((resolve) => {
          setTimeout(() => {
            this._executeWithRetry().then(resolve);
          }, backoffDelay);
        });
      } else {
        logWithTimestamp(`[${this.taskName}] Max retries (${this.options.maxRetries}) exceeded. Giving up.`);
        this._currentRetry = 0;
      }
    }

    // Schedule next run if still running
    this._scheduleNextRun();
  }

  /**
   * Check if the task is currently running
   */
  get isRunning() {
    return this._isRunning;
  }

  /**
   * Get the timestamp of the last run
   */
  getLastRunTimestamp() {
    return this._lastRunTimestamp;
  }

  /**
   * Get the count of successful executions
   */
  getSuccessCount() {
    return this._successCount;
  }

  /**
   * Get the count of failed executions
   */
  getFailureCount() {
    return this._failureCount;
  }
}

/**
 * Factory function to create a ScheduledTask instance with environment-based defaults
 * @param {Function} handlerFunction - The function to execute on schedule
 * @param {Object} options - Optional configuration overrides
 * @returns {ScheduledTask} A configured ScheduledTask instance
 */
function createScheduledTask(handlerFunction, options = {}) {
  const taskName = options.taskName || process.env.TASK_NAME || 'unknown';
  const intervalMs = options.intervalMs || parseInt(process.env.SCHEDULE_INTERVAL_MS, 10) || 300000; // 5 minutes default
  const immediate = options.immediate !== undefined ? options.immediate : process.env.SCHEDULE_IMMEDIATE === 'true';
  const maxRetries = options.maxRetries !== undefined ? options.maxRetries : 3;

  const taskOptions = {
    immediate,
    maxRetries
  };

  return new ScheduledTask(taskName, intervalMs, handlerFunction, taskOptions);
}

/**
 * Gracefully shutdown multiple scheduled tasks
 * @param {ScheduledTask[]} tasks - Array of ScheduledTask instances to stop
 * @param {number} timeoutMs - Maximum time to wait for graceful shutdown (default: 30000ms / 30 seconds)
 * @returns {Promise<void>} Resolves when all tasks have stopped or timeout is reached
 */
function gracefulShutdown(tasks, timeoutMs = 30000) {
  return new Promise((resolve, reject) => {
    logWithTimestamp(`[gracefulShutdown] Initiating graceful shutdown for ${tasks.length} tasks`);

    if (tasks.length === 0) {
      logWithTimestamp('[gracefulShutdown] No tasks to stop');
      resolve();
      return;
    }

    // Stop all tasks
    tasks.forEach(task => {
      if (task.isRunning) {
        task.stop();
      }
    });

    // Wait for all tasks to stop with a timeout
    const timeout = setTimeout(() => {
      const runningTasks = tasks.filter(task => task.isRunning);
      if (runningTasks.length > 0) {
        logWithTimestamp(`[gracefulShutdown] Timeout reached. Forcing shutdown of ${runningTasks.length} remaining tasks`);
      }
      resolve();
    }, timeoutMs);

    // Check periodically if all tasks have stopped
    const checkInterval = setInterval(() => {
      const runningTasks = tasks.filter(task => task.isRunning);
      if (runningTasks.length === 0) {
        clearTimeout(timeout);
        clearInterval(checkInterval);
        logWithTimestamp('[gracefulShutdown] All tasks stopped gracefully');
        resolve();
      }
    }, 100);

    // Cleanup on first completion
    const originalResolve = resolve;
    resolve = () => {
      clearTimeout(timeout);
      clearInterval(checkInterval);
      originalResolve();
    };
  });
}

// Export the scheduler module
module.exports = {
  ScheduledTask,
  createScheduledTask,
  gracefulShutdown
};
