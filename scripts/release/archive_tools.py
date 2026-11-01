"""Shared archive readers for debug package verification.
调试包验证共享的归档读取工具。
"""

from pathlib import Path
import tarfile


def archive_files(path):
    """Read only safe regular files from a release archive; return exact name-to-bytes mapping.
    仅从发布归档读取安全普通文件；返回精确名称到字节映射。
    """
    # Result rejects duplicate names so a later extraction cannot change the validated identity.
    # 结果拒绝重复名称，确保后续解压不能改变已验证身份。
    result = {}
    with tarfile.open(path, "r:gz") as archive:
        for member in archive:
            if member.name in result or member.name.startswith("/") or ".." in Path(member.name).parts or "\\" in member.name:
                raise ValueError(f"Unsafe release archive member: {member.name}")
            if member.isdir():
                continue
            if not member.isfile():
                raise ValueError(f"Nonregular release archive member: {member.name}")
            result[member.name] = archive.extractfile(member).read()
    return result
