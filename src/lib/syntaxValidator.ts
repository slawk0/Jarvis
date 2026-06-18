import yaml from 'js-yaml';
import toml from 'toml';

/**
 * Validates the syntax of the code in the editor model.
 * Currently supports: JSON, YAML, TOML, XML, INI.
 * Adds visual red markers inside Monaco Editor and returns the error message if any.
 *
 * @param monaco The monaco instance
 * @param model The editor model
 * @param filePath The file path, used to detect extension
 * @returns The syntax error message, or null if syntax is valid / unsupported extension
 */
export function validateContent(
  monaco: any,
  model: any,
  filePath: string
): string | null {
  if (!model) return null;
  const content = model.getValue();
  const markers: any[] = [];
  let syntaxError: string | null = null;

  const ext = filePath.split('.').pop()?.toLowerCase();

  if (ext === 'json') {
    try {
      JSON.parse(content);
    } catch (e: any) {
      let line = 1;
      let column = 1;
      
      const lineColMatch = e.message.match(/line (\d+) column (\d+)/i);
      if (lineColMatch) {
        line = parseInt(lineColMatch[1], 10);
        column = parseInt(lineColMatch[2], 10);
      } else {
        const posMatch = e.message.match(/at position (\d+)/i);
        if (posMatch) {
          const pos = parseInt(posMatch[1], 10);
          const before = content.substring(0, pos);
          const lines = before.split('\n');
          line = lines.length;
          column = lines[lines.length - 1].length + 1;
        }
      }

      syntaxError = e.message;
      markers.push({
        message: e.message,
        severity: monaco.MarkerSeverity.Error,
        startLineNumber: line,
        startColumn: column,
        endLineNumber: line,
        endColumn: column + 10,
      });
    }
  } else if (ext === 'yaml' || ext === 'yml') {
    try {
      yaml.load(content);
    } catch (e: any) {
      syntaxError = e.message;
      markers.push({
        message: e.message,
        severity: monaco.MarkerSeverity.Error,
        startLineNumber: e.mark ? e.mark.line + 1 : 1,
        startColumn: e.mark ? e.mark.column + 1 : 1,
        endLineNumber: e.mark ? e.mark.line + 1 : 1,
        endColumn: e.mark ? e.mark.column + 100 : 100,
      });
    }
  } else if (ext === 'toml') {
    try {
      toml.parse(content);
    } catch (e: any) {
      syntaxError = e.message;
      markers.push({
        message: e.message,
        severity: monaco.MarkerSeverity.Error,
        startLineNumber: e.line || 1,
        startColumn: e.column || 1,
        endLineNumber: e.line || 1,
        endColumn: (e.column || 1) + 10,
      });
    }
  } else if (ext === 'xml') {
    try {
      const parser = new DOMParser();
      const doc = parser.parseFromString(content, 'application/xml');
      const parserError = doc.querySelector('parsererror');
      if (parserError) {
        const msg = parserError.textContent || 'XML Parsing Error';
        syntaxError = msg;
        
        let line = 1;
        let column = 1;
        const lineMatch = msg.match(/line\s+(\d+)/i);
        const colMatch = msg.match(/column\s+(\d+)/i);
        if (lineMatch) line = parseInt(lineMatch[1], 10);
        if (colMatch) column = parseInt(colMatch[1], 10);

        markers.push({
          message: msg,
          severity: monaco.MarkerSeverity.Error,
          startLineNumber: line,
          startColumn: column,
          endLineNumber: line,
          endColumn: column + 10,
        });
      }
    } catch (e: any) {
      syntaxError = e.message;
      markers.push({
        message: e.message,
        severity: monaco.MarkerSeverity.Error,
        startLineNumber: 1,
        startColumn: 1,
        endLineNumber: 1,
        endColumn: 100,
      });
    }
  } else if (ext === 'ini') {
    try {
      const lines = content.split('\n');
      for (let i = 0; i < lines.length; i++) {
        const lineText = lines[i].trim();
        if (!lineText || lineText.startsWith(';') || lineText.startsWith('#')) {
          continue;
        }
        if (lineText.startsWith('[') && lineText.endsWith(']')) {
          continue;
        }
        if (lineText.includes('=')) {
          continue;
        }
        throw {
          message: 'Syntax error: Line must be a comment (; or #), a section [header], or a key = value pair',
          line: i + 1,
          column: 1
        };
      }
    } catch (e: any) {
      syntaxError = e.message;
      markers.push({
        message: e.message,
        severity: monaco.MarkerSeverity.Error,
        startLineNumber: e.line || 1,
        startColumn: e.column || 1,
        endLineNumber: e.line || 1,
        endColumn: 100,
      });
    }
  }

  monaco.editor.setModelMarkers(model, "syntax-validation", markers);
  return syntaxError;
}
