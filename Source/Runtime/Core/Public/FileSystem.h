#pragma once

#include <map>
#include <string>
#include <vector>

namespace ArenaBoss::FileSystem
{
    std::string ReadFile(const std::string& path);
    std::vector<char> ReadBinary(const std::string& path);

    std::string GetCurrentPath();
    bool IsExist(const std::string& path);

    std::string GetFileName(const std::string& path);
    std::vector<std::string> GetFileNames(const std::string& dir);
    std::vector<std::string> GetFileNames(const std::string& dir, const std::string& ext);
}