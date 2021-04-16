#include "ConfigFile.h"
#include <algorithm>
#include "Log.h"

const String* ConfigFile::ConfigSection::operator[](const String& key) const noexcept
{
    const auto iter = data.find(key);
    return iter != data.cend() ? &(iter->second) : nullptr;
}

namespace
{
    String TrimInline(const String& s) noexcept
    {
        auto begin = s.find_first_not_of(u" \t");
        auto end = s.find_last_not_of(u" \t");

        if (begin == String::npos || end == String::npos || begin > end)
            return u"";
        
        return s.substr(begin, end - begin + 1);
    }
}

bool ConfigFile::LoadFromFile(const String& fileName) noexcept
{
    Clear();

    const auto path = STR("Config\\") + fileName + STR(".ini");
    IfStream fin{ reinterpret_cast<const BSBase::uint16*>(path.c_str()) };
    if (!fin) return false;

    String section{ u"" };
    std::unordered_map<String, String> sectionMap;
    String line;

    while (std::getline(fin, line))
    {
        if ((line = TrimInline(line)).empty() || line[0] == STR('#'))
            continue;

        if (line[0] == STR('['))
        {
            const auto end = line.rfind(STR(']'));
            if (end == String::npos || end == 1)
            {
                Clear();
                return false;
            }

            data.emplace(std::make_pair(section, ConfigSection{ sectionMap }));
            section = TrimInline(line.substr(1, end - 1));
        }
        else
        {
            const auto eq = line.find(STR('='));
            if (eq == String::npos || eq == 0 || eq == line.length() - 1)
            {
                Clear();
                return false;
            }

            const auto left = TrimInline(line.substr(0, eq));
            const auto right = TrimInline(line.substr(eq + 1));

            if (left.empty() || right.empty())
            {
                Clear();
                return false;
            }

            sectionMap.emplace(std::make_pair(left, right));
        }
    }

    data.emplace(std::make_pair(section, ConfigSection{ sectionMap }));
    isAvailable = true;
    return true;
}

void ConfigFile::Clear() noexcept
{
    data.clear();
    isAvailable = false;
}

const String* ConfigFile::operator()(const String& sectionName, const String& keyName) const noexcept
{
    const auto iter = data.find(sectionName);
    if (iter == data.cend()) return nullptr;
    return (iter->second)[keyName];
}
