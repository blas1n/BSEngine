#pragma once

#include <>
#include <utility>

namespace ArenaBoss
{
	template <class... Args>
	void Log(const char* format, Args&&... args)
	{
		//SDL_Log(format, std::forward<Args>(args)...);
	}
}