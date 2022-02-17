#pragma once

#include <unordered_map>
#include "BSBase/Type.h"
#include "CharSet.h"

class ConfigFile final
{
    class ConfigSection final
    {
    public:
        ConfigSection(const std::unordered_map<String, String>& inData)
            : data(inData) {}

        ConfigSection(std::unordered_map<String, String>&& inData) noexcept
            : data(std::move(inData)) {}
        
        [[nodiscard]] const String* operator[](const String& key) const noexcept;

    private:
        std::unordered_map<String, String> data;
    };

public:
    CORE_API  ConfigFile() = default;

    CORE_API ConfigFile(const String& fileName)
        : ConfigFile{}
    {
        LoadFromFile(fileName);
    }

    CORE_API bool LoadFromFile(const String& fileName) noexcept;
    CORE_API void Clear() noexcept;

    [[nodiscard]] CORE_API bool IsAvailable() const noexcept { return isAvailable; }
    [[nodiscard]] CORE_API operator bool() const noexcept { return isAvailable; }

    [[nodiscard]] CORE_API bool IsExistSection(const String& section) const noexcept
    {
        return data.find(section) != data.cend();
    }

    [[nodiscard]] CORE_API const String* operator()(const String& sectionName, const String& keyName) const noexcept;

private:
    std::unordered_map<String, ConfigSection> data;
    BSBase::uint8 isAvailable : 1;
};
