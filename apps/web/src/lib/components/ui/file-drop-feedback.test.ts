/** @vitest-environment happy-dom */
import { describe, expect, it } from 'vitest';
import { fileDropFeedback } from './file-drop-feedback';

function fileDragEvent(type: string): DragEvent {
  const event = new Event(type, { bubbles: true }) as DragEvent;
  Object.defineProperty(event, 'dataTransfer', {
    value: { files: [], types: ['Files'] },
  });
  return event;
}

describe('fileDropFeedback', () => {
  it('keeps the drop target active while a file moves between descendants', () => {
    const target = document.createElement('div');
    const child = document.createElement('div');
    target.append(child);
    const action = fileDropFeedback(target);

    target.dispatchEvent(fileDragEvent('dragenter'));
    child.dispatchEvent(fileDragEvent('dragenter'));
    child.dispatchEvent(fileDragEvent('dragleave'));

    expect(target.classList.contains('file-drop-feedback--active')).toBe(true);

    target.dispatchEvent(fileDragEvent('dragleave'));
    expect(target.classList.contains('file-drop-feedback--active')).toBe(false);

    action.destroy();
  });

  it('clears the feedback after a drop', () => {
    const target = document.createElement('div');
    const action = fileDropFeedback(target);

    target.dispatchEvent(fileDragEvent('dragenter'));
    target.dispatchEvent(fileDragEvent('drop'));

    expect(target.classList.contains('file-drop-feedback--active')).toBe(false);
    action.destroy();
  });
});
