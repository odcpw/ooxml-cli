export const uploadSupportedExtensions = ['.pptx', '.pptm', '.docx', '.xlsx', '.xlsm'] as const;
export const uploadAcceptAttribute = uploadSupportedExtensions.join(',');

export const previewSupportedExtensions = ['.pptx', '.pptm'] as const;
export const previewSupportedLabel = 'PPTX/PPTM';

const uploadSupportedExtensionSet = new Set<string>(uploadSupportedExtensions);
const previewSupportedExtensionSet = new Set<string>(previewSupportedExtensions);

export function isUploadExtensionSupported(extension: string): boolean {
  return uploadSupportedExtensionSet.has(extension.toLowerCase());
}

export function isPreviewExtensionSupported(extension: string): boolean {
  return previewSupportedExtensionSet.has(extension.toLowerCase());
}

export const previewInspectCopy = `Render ${previewSupportedLabel} thumbnails to inspect output.`;
export const previewAvailableOnlyCopy = `Preview thumbnails are currently available for ${previewSupportedLabel} only.`;
export const previewWiredOnlyCopy = `Preview thumbnails are currently wired for ${previewSupportedLabel}.`;
export const previewRenderPromptCopy = `Click Render preview for ${previewSupportedLabel} files.`;
export const previewUnavailableReasonCopy = `Preview rendering is currently wired for ${previewSupportedLabel} only.`;
