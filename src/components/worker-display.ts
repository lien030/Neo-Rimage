export function inputFileName(inputPath: string): string {
  const withoutTrailingSeparators = inputPath.replace(/[\\/]+$/, "");
  const separatorIndex = Math.max(
    withoutTrailingSeparators.lastIndexOf("/"),
    withoutTrailingSeparators.lastIndexOf("\\"),
  );
  const fileName = withoutTrailingSeparators.slice(separatorIndex + 1);

  return fileName || inputPath;
}
