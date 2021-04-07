#pragma once

#include <unordered_map>
#include "BSBase/Type.h"
#include "CharSet.h"

class CORE_API ConfigFile final
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
    ConfigFile() = default;

    ConfigFile(const String& fileName)
        : ConfigFile{}
    {
        LoadFromFile(fileName);
    }

    bool LoadFromFile(const String& fileName) noexcept;
    void Clear() noexcept;

    [[nodiscard]] bool IsAvailable() const noexcept { return isAvailable; }
    [[nodiscard]] operator bool() const noexcept { return isAvailable; }

    [[nodiscard]] bool IsExistSection(const String& section) const noexcept
    {
        return data.find(section) != data.cend();
    }

    [[nodiscard]] const String* operator()(const String& sectionName, const String& keyName) const noexcept;

private:
    std::unordered_map<String, ConfigSection> data;
    BSBase::uint8 isAvailable : 1;
};
