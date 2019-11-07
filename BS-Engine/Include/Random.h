#pragma once

#include <random>
#include "Core.h"

class BS_API Random
{
public:
	static inline void Init() noexcept
	{
		std::random_device rd;
		Seed(rd());
	}

	static inline void Seed(uint32 seed) noexcept
	{
		generator.seed(seed);
	}

	static inline bool GetBool() noexcept
	{
		return GetRange(0, 1) == 1;
	}

	static inline float GetFloat() noexcept
	{
		return GetRange(0.0f, 1.0f);
	}

	static inline int GetRange(const int min, const int max) noexcept
	{
		std::uniform_int_distribution<int> dist{ min, max };
		return dist(generator);
	}

	static inline float GetRange(const float min, const float max) noexcept
	{
		std::uniform_real_distribution<float> dist(min, max);
		return dist(generator);
	}

	static class Vector2 GetRange(const Vector2& min, const Vector2& max) noexcept;
	static class Vector3 GetRange(const Vector3& min, const Vector3& max) noexcept;
	static class Vector4 GetRange(const Vector4& min, const Vector4& max) noexcept;

private:
	static std::mt19937 generator;
};