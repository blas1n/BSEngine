#include "FileSystem.h"
#include <filesystem>
#include <fstream>
#include <iterator>

namespace ArenaBoss::FileSystem
{
    namespace
    {
        namespace fs = std::filesystem;
    }

    std::string ReadFile(const std::string& path)
    {
        std::ifstream in{ path };
        if (!in)
            throw "Cannot open file.";

        return std::string{ std::istreambuf_iterator<char>{ in },
            std::istreambuf_iterator<char>{} };
    }

    std::vector<char> ReadBinary(const std::string& path)
    {
        std::ifstream in{ path, std::ios::binary };
        if (!in)
            throw "Cannot open file.";

        auto buf = in.rdbuf();
        auto fileSize = buf->pubseekoff(0, std::ios::end, std::ios::in);
        buf->pubseekpos(0, std::ios::in);

        std::vector<char> ret(static_cast<size_t>(fileSize));
        buf->sgetn(ret.data(), fileSize);
        return ret;
    }

    std::string GetCurrentPath()
    {
        return fs::current_path().string();
    }

    bool IsExist(const std::string& path)
    {
        return fs::exists(fs::path{ path });
    }

    std::string GetFileName(const std::string& path)
    {
        return fs::path{ path }.filename().string();
    }

    std::vector<std::string> GetFileNames(const std::string& dir)
    {
        std::vector<std::string> ret;

        for (auto&& entry : fs::directory_iterator{ dir })
        {
            auto& path = entry.path();
            if (fs::is_regular_file(path))
                ret.push_back(path.string());
        }

        return ret;
    }

    std::vector<std::string> GetFileNames(const std::string& dir, const std::string& ext)
    {
        std::vector<std::string> ret;

        for (auto&& entry : fs::directory_iterator{ dir })
        {
            auto& path = entry.path();

            auto pathExt = path.extension().string();

            if (fs::is_regular_file(path) && pathExt == ext)
                ret.push_back(path.string());
        }

        return ret;
    }

    bool ReadFileBinary(const std::wstring& filename, std::vector<char>& buf)
    {
        buf.clear();

        std::ifstream fin(filename, std::ios::binary);
        if (!fin)
            return false;

        std::filebuf* fb = fin.rdbuf();
        std::streampos fileSize = fb->pubseekoff(0, std::ios::end, std::ios::in);
        fb->pubseekpos(0, std::ios::in);

        buf.resize(static_cast<size_t>(fileSize));
        fb->sgetn(buf.data(), fileSize);

        return true;
    }
}