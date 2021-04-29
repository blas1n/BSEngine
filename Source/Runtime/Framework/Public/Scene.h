#pragma once

#include "Name.h"

class FRAMEWORK_API Scene final
{
public:
	Scene() = default;

	Scene(const Scene&) = delete;
	Scene(Scene&&) noexcept = default;

	Scene& operator=(const Scene&) = delete;
	Scene& operator=(Scene&&) noexcept = default;

	~Scene() { Release(); }

	void Init(Name inName);
	void Release() noexcept;

	void Load();
	void Save();

private:
	Name name;
};
