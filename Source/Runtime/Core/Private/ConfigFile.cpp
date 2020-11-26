#include "ConfigFile.h"
#include <algorithm>
#include <fstream>
#include "Log.h"

const std::string* ConfigFile::ConfigSection::operator[](const std::string& key) const noexcept
{
    const auto iter = data.find(key);
    return iter != data.cend() ? &(iter->second) : nullptr;
}

namespace
{
    std::string TrimInline(const std::string& s)
    {
        auto begin = s.find_first_not_of(" \t");
        auto end = s.find_last_not_of(" \t");
        if (begin == std::string::npos || end == std::string::npos || begin > end)
            return "";
        return s.substr(begin, end - begin + 1);
    }
}

bool ConfigFile::LoadFromFile(const std::string& fileName) noexcept
{
    if (state != State::Uninitialized)
        throw std::exception{ "Not initialized" };

    try
    {
        std::ifstream fin("Config\\" + fileName + ".ini");
        if (!fin) return false;

        std::string section{ "Global" };
        std::unordered_map<std::string, std::string> sectionMap;
        std::string line;

        while (std::getline(fin, line))
        {
            if ((line = TrimInline(line)).empty() || line[0] == '#')
                continue;

            if (line[0] == '[')
            {
                const auto end = line.rfind(']');
                if (end == std::string::npos || end == 1)
                    throw;

                data.emplace(std::make_pair(section, ConfigSection{ sectionMap }));
                section = TrimInline(line.substr(1, end - 1));
            }
            else
            {
                const auto eq = line.find('=');
                if (eq == std::string::npos || eq == 0 || eq == line.length() - 1)
                    throw;

                const auto left = TrimInline(line.substr(0, eq));
                const auto right = TrimInline(line.substr(eq + 1));

                if (left.empty() || right.empty())
                    throw;

                sectionMap.emplace(std::make_pair(left, right));
            }
        }

        data.emplace(std::make_pair(section, ConfigSection{ sectionMap }));
        state = State::Available;
        return true;
    }
    catch (...)
    {
        data.clear();
        return false;
    }
}

void ConfigFile::Clear() noexcept
{
    data.clear();
    state = State::Uninitialized;
}

const std::string* ConfigFile::operator()(const std::string& sectionName, const std::string& keyName) const noexcept
{
    const auto iter = data.find(sectionName);
    if (iter == data.cend()) return nullptr;
    return (iter->second)[keyName];
}