#pragma once

#include <string>

struct LogCategory
{
	std::string name;
};

#define DECLARE_LOG_CATEGORY(name) extern const LogCategory name;
#define DEFINE_LOG_CATEGORY(name) const LogCategory name{ #name };
