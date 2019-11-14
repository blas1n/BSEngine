#pragma once

#include <random>
#include "Core.h"

/// @brief Random factory with 'Mersenne Twister'
class BS_API Random
{
public:
	/// @brief Random class initialization.
	static inline void Init() noexcept
	{
		std::random_device rd;
		Seed(rd());
	}

	/// @brief Set seed.
	static inline void Seed(uint32 seed) noexcept
	{
		generator.seed(seed);
	}

	/// @brief Get random bool.
	static inline bool GetBool() noexcept
	{
		return GetRange(0, 1) == 1;
	}

	/// @brief Get random decimal between 0 and 1.
	static inline float GetFloat() noexcept
	{
		return GetRange(0.0f, 1.0f);
	}

	/// @brief Get random integer between min and max.
	static inline int GetRange(const int min, const int max) noexcept
	{
		std::uniform_int_distribution<int> dist{ min, max };
		return dist(generator);
	}

	/// @brief Get random decimal between min and max.
	static inline float GetRange(const float min, const float max) noexcept
	{
		std::uniform_real_distribution<float> dist(min, max);
		return dist(generator);
	}

	/// @brief Get random two dimensional vector between min and max.
	static class Vector2 GetRange(const Vector2& min, const Vector2& max) noexcept;

	/// @brief Get random three dimensional vector between min and max.
	static class Vector3 GetRange(const Vector3& min, const Vector3& max) noexcept;

	/// @brief Get random four dimensional vector between min and max.
	static class Vector4 GetRange(const Vector4& min, const Vector4& max) noexcept;

private:
	static std::mt19937 generator;
};