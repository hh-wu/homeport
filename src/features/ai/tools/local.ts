import {
  FilePlus2,
  FileText,
  FolderOpen,
  Pencil,
  SearchCode,
  SquareTerminal,
} from "lucide-react";

import { ipc } from "@/lib/ipc";
import { errorMessage } from "@/lib/toast";
import {
  num,
  str,
  bool,
  toolFailure,
  toolSuccess,
  type AiTool,
  type ToolExecutionResult,
} from "./types";

async function listDirectory(
  args: Record<string, unknown>,
): Promise<ToolExecutionResult> {
  const path = str(args, "path");
  if (!path) return toolFailure("Error: path is required.");
  try {
    const entries = await ipc.localFs.listDirectory(path);
    if (entries.length === 0) return toolSuccess("The directory is empty.");
    return toolSuccess(JSON.stringify(entries));
  } catch (error) {
    return toolFailure(`Error: ${errorMessage(error)}`);
  }
}

async function readFile(
  args: Record<string, unknown>,
): Promise<ToolExecutionResult> {
  const path = str(args, "path");
  if (!path) return toolFailure("Error: path is required.");
  try {
    const file = await ipc.localFs.readFile(
      path,
      num(args, "offset"),
      num(args, "limit"),
    );
    const header = file.truncated
      ? `(${file.totalLines} lines total, showing a range; read further ranges with offset)`
      : `(${file.totalLines} lines total)`;
    return toolSuccess(`${header}\n${file.content}`);
  } catch (error) {
    return toolFailure(`Error: ${errorMessage(error)}`);
  }
}

async function searchFiles(
  args: Record<string, unknown>,
): Promise<ToolExecutionResult> {
  const root = str(args, "root");
  const pattern = str(args, "pattern");
  if (!root || !pattern) {
    return toolFailure("Error: root and pattern are required.");
  }
  try {
    const hits = await ipc.localFs.searchFiles(root, pattern, num(args, "max"));
    if (hits.length === 0) return toolSuccess("No matches found.");
    return toolSuccess(
      hits.map((hit) => `${hit.path}:${hit.line}: ${hit.text}`).join("\n"),
    );
  } catch (error) {
    return toolFailure(`Error: ${errorMessage(error)}`);
  }
}

async function writeFile(
  args: Record<string, unknown>,
): Promise<ToolExecutionResult> {
  const path = str(args, "path");
  const content = str(args, "content");
  if (!path) return toolFailure("Error: path is required.");
  try {
    await ipc.localFs.writeFile(path, content);
    return toolSuccess(`Wrote ${content.length} bytes to ${path}.`);
  } catch (error) {
    return toolFailure(`Error: ${errorMessage(error)}`);
  }
}

async function editFile(
  args: Record<string, unknown>,
): Promise<ToolExecutionResult> {
  const path = str(args, "path");
  const oldString = str(args, "oldString");
  if (!path || !oldString) {
    return toolFailure("Error: path and oldString are required.");
  }
  try {
    const replaced = await ipc.localFs.editFile(
      path,
      oldString,
      str(args, "newString"),
      bool(args, "replaceAll") ? true : undefined,
    );
    return toolSuccess(
      `Edited ${path} (${replaced} replacement${replaced > 1 ? "s" : ""}).`,
    );
  } catch (error) {
    return toolFailure(`Error: ${errorMessage(error)}`);
  }
}

async function runLocalCommand(
  args: Record<string, unknown>,
): Promise<ToolExecutionResult> {
  const command = str(args, "command");
  if (!command) return toolFailure("Error: command is required.");
  try {
    const result = await ipc.localFs.runCommand(
      command,
      num(args, "timeoutSecs"),
    );
    if (result.timedOut) {
      return toolFailure(`Error: ${result.output}`);
    }
    const header = `exit code ${result.exitCode}`;
    return toolSuccess(
      result.output ? `${header}\n${result.output}` : `${header}\n(no output)`,
    );
  } catch (error) {
    return toolFailure(`Error: ${errorMessage(error)}`);
  }
}

const PATH_FIELD = {
  path: {
    type: "string",
    description: "Absolute path inside the allowed local directories.",
  },
} as const;

export const localTools: AiTool[] = [
  {
    spec: {
      name: "local_list_directory",
      description:
        "List a directory on this machine (dirs first, with sizes and modified times). Use it to explore the local file system before reading or editing.",
      parameters: {
        type: "object",
        properties: PATH_FIELD,
        required: ["path"],
        additionalProperties: false,
      },
    },
    icon: FolderOpen,
    labelKey: "ai.tool.listLocalDirectory",
    execute: listDirectory,
  },
  {
    spec: {
      name: "local_read_file",
      description:
        "Read a UTF-8 text file on this machine. Large files are returned in ranges; use offset to continue reading.",
      parameters: {
        type: "object",
        properties: {
          ...PATH_FIELD,
          offset: {
            type: "number",
            description: "Zero-based line number to start from.",
          },
          limit: {
            type: "number",
            description: "Maximum lines to return (default 400, max 2000).",
          },
        },
        required: ["path"],
        additionalProperties: false,
      },
    },
    icon: FileText,
    labelKey: "ai.tool.readLocalFile",
    execute: readFile,
  },
  {
    spec: {
      name: "local_search_files",
      description:
        "Search file contents under a local directory for a literal, case-insensitive substring and return matching lines with file paths and line numbers.",
      parameters: {
        type: "object",
        properties: {
          root: {
            type: "string",
            description: "Directory to search recursively.",
          },
          pattern: {
            type: "string",
            description: "Literal text to search for (not a regex).",
          },
          max: {
            type: "number",
            description: "Maximum hits to return (default 50, max 200).",
          },
        },
        required: ["root", "pattern"],
        additionalProperties: false,
      },
    },
    icon: SearchCode,
    labelKey: "ai.tool.searchLocalFiles",
    execute: searchFiles,
  },
  {
    spec: {
      name: "local_write_file",
      description:
        "Write a whole UTF-8 text file on this machine, creating or overwriting it. Prefer edit_file for targeted changes to existing files.",
      parameters: {
        type: "object",
        properties: {
          ...PATH_FIELD,
          content: {
            type: "string",
            description: "The full file content to write.",
          },
        },
        required: ["path", "content"],
        additionalProperties: false,
      },
    },
    icon: FilePlus2,
    labelKey: "ai.tool.writeLocalFile",
    requiresApproval: true,
    confirmKey: "ai.confirmWriteLocalFile",
    execute: writeFile,
  },
  {
    spec: {
      name: "local_edit_file",
      description:
        "Replace an exact text span in a local file. Fails when oldString is absent or matches more than once unless replaceAll is set.",
      parameters: {
        type: "object",
        properties: {
          ...PATH_FIELD,
          oldString: {
            type: "string",
            description: "The exact text to replace.",
          },
          newString: {
            type: "string",
            description: "The replacement text (may be empty).",
          },
          replaceAll: {
            type: "boolean",
            description:
              "Replace every match instead of requiring a unique one.",
          },
        },
        required: ["path", "oldString", "newString"],
        additionalProperties: false,
      },
    },
    icon: Pencil,
    labelKey: "ai.tool.editLocalFile",
    requiresApproval: true,
    confirmKey: "ai.confirmEditLocalFile",
    execute: editFile,
  },
  {
    spec: {
      name: "run_local_command",
      description:
        "Run a PowerShell command on this machine and return the exit code and combined output. Use for local diagnostics, launches, and scripts that do not fit other tools.",
      parameters: {
        type: "object",
        properties: {
          command: {
            type: "string",
            description: "The PowerShell command line to run.",
          },
          timeoutSecs: {
            type: "number",
            description: "Timeout in seconds (default 120, max 600).",
          },
        },
        required: ["command"],
        additionalProperties: false,
      },
    },
    icon: SquareTerminal,
    labelKey: "ai.tool.runLocalCommand",
    requiresApproval: true,
    confirmKey: "ai.confirmRunLocal",
    execute: runLocalCommand,
  },
];
