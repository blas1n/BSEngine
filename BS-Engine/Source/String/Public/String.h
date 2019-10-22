#pragma once

#include "Core.h"

class BS_API String final
{
public:
	String() = default;
	String(const String&) = default;
	String(String&&) = default;
	String& operator=(const String&) = default;
	String& operator=(String&&) = default;

	String(const tchar* src);
	explicit String(int32 count, tchar src);
private:
};