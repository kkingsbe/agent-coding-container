const fs = require('fs').promises;
const path = require('path');
const os = require('os');

/**
 * WorkspaceManager class for managing workspace file operations
 * Handles reading and writing markdown files with atomic operations
 */
class WorkspaceManager {
  /**
   * Create a WorkspaceManager instance
   * @param {string} workspacePath - Path to the workspace directory
   */
  constructor(workspacePath = process.env.WORKSPACE_PATH || '/workspace') {
    this.workspacePath = workspacePath;
  }

  /**
   * Get the workspace path
   * @returns {string} The workspace path
   */
  getWorkspacePath() {
    return this.workspacePath;
  }

  /**
   * Check if a file exists
   * @param {string} filePath - Path to the file (relative to workspace or absolute)
   * @returns {Promise<boolean>} True if file exists, false otherwise
   */
  async fileExists(filePath) {
    try {
      const fullPath = path.isAbsolute(filePath)
        ? filePath
        : path.join(this.workspacePath, filePath);
      await fs.access(fullPath, fs.constants.F_OK);
      return true;
    } catch (error) {
      return false;
    }
  }

  /**
   * Ensure a directory exists, create it if it doesn't
   * @param {string} dirPath - Path to the directory
   * @returns {Promise<void>}
   */
  async ensureDirectory(dirPath) {
    try {
      const fullPath = path.isAbsolute(dirPath)
        ? dirPath
        : path.join(this.workspacePath, dirPath);
      await fs.mkdir(fullPath, { recursive: true });
    } catch (error) {
      if (error.code !== 'EEXIST') {
        throw new Error(`Failed to ensure directory ${dirPath}: ${error.message}`);
      }
    }
  }

  /**
   * Read a markdown file
   * @param {string} filePath - Path to the markdown file
   * @returns {Promise<string>} Content of the file
   */
  async readMarkdownFile(filePath) {
    try {
      const fullPath = path.isAbsolute(filePath)
        ? filePath
        : path.join(this.workspacePath, filePath);
      const content = await fs.readFile(fullPath, 'utf-8');
      return content;
    } catch (error) {
      throw new Error(`Failed to read markdown file ${filePath}: ${error.message}`);
    }
  }

  /**
   * Read TODO.md from workspace
   * @returns {Promise<string>} Content of TODO.md
   */
  async readTodoFile() {
    return this.readMarkdownFile('TODO.md');
  }

  /**
   * Read BACKLOG.md from workspace
   * @returns {Promise<string>} Content of BACKLOG.md
   */
  async readBacklogFile() {
    return this.readMarkdownFile('BACKLOG.md');
  }

  /**
   * Read COMPLETED.md from workspace
   * @returns {Promise<string>} Content of COMPLETED.md
   */
  async readCompletedFile() {
    return this.readMarkdownFile('COMPLETED.md');
  }

  /**
   * Read BLOCKERS.md from workspace
   * @returns {Promise<string>} Content of BLOCKERS.md
   */
  async readBlockersFile() {
    return this.readMarkdownFile('BLOCKERS.md');
  }

  /**
   * Read PRD.md from workspace
   * @returns {Promise<string>} Content of PRD.md
   */
  async readPrdFile() {
    return this.readMarkdownFile('PRD.md');
  }

  /**
   * Write content to a markdown file using atomic operation
   * @param {string} filePath - Path to the markdown file
   * @param {string} content - Content to write
   * @returns {Promise<void>}
   */
  async writeMarkdownFile(filePath, content) {
    try {
      const fullPath = path.isAbsolute(filePath)
        ? filePath
        : path.join(this.workspacePath, filePath);
      
      // Ensure directory exists
      const dirPath = path.dirname(fullPath);
      await this.ensureDirectory(dirPath);

      // Create temp file in same directory for atomic rename
      const tempFile = path.join(
        path.dirname(fullPath),
        `.${path.basename(fullPath)}.tmp-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`
      );

      // Write to temp file first
      await fs.writeFile(tempFile, content, 'utf-8');

      // Rename temp file to target file (atomic operation on most filesystems)
      await fs.rename(tempFile, fullPath);
    } catch (error) {
      throw new Error(`Failed to write markdown file ${filePath}: ${error.message}`);
    }
  }

  /**
   * Write content to TODO.md
   * @param {string} content - Content to write
   * @returns {Promise<void>}
   */
  async writeTodoFile(content) {
    return this.writeMarkdownFile('TODO.md', content);
  }

  /**
   * Write content to BACKLOG.md
   * @param {string} content - Content to write
   * @returns {Promise<void>}
   */
  async writeBacklogFile(content) {
    return this.writeMarkdownFile('BACKLOG.md', content);
  }

  /**
   * Write content to COMPLETED.md
   * @param {string} content - Content to write
   * @returns {Promise<void>}
   */
  async writeCompletedFile(content) {
    return this.writeMarkdownFile('COMPLETED.md', content);
  }

  /**
   * Write content to BLOCKERS.md
   * @param {string} content - Content to write
   * @returns {Promise<void>}
   */
  async writeBlockersFile(content) {
    return this.writeMarkdownFile('BLOCKERS.md', content);
  }

  /**
   * Parse markdown content into sections by ## headings
   * @param {string} content - Markdown content to parse
   * @returns {Object} Object with section names as keys and content as values
   */
  parseMarkdownSections(content) {
    const sections = {};
    const lines = content.split('\n');
    let currentSection = '';
    let currentContent = [];

    for (const line of lines) {
      if (line.trim().startsWith('## ')) {
        // Save previous section
        if (currentSection || currentContent.length > 0) {
          sections[currentSection] = currentContent.join('\n').trim();
        }
        // Start new section
        currentSection = line.trim().substring(3).trim();
        currentContent = [];
      } else {
        currentContent.push(line);
      }
    }

    // Save last section
    if (currentSection || currentContent.length > 0) {
      sections[currentSection] = currentContent.join('\n').trim();
    }

    return sections;
  }

  /**
   * Extract a specific section from markdown content
   * @param {string} content - Markdown content
   * @param {string} sectionName - Name of the section to extract
   * @returns {string|null} Content of the section or null if not found
   */
  extractSection(content, sectionName) {
    const sections = this.parseMarkdownSections(content);
    return sections[sectionName] || null;
  }
}

module.exports = WorkspaceManager;
