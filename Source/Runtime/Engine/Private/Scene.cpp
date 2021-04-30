#include "Scene.h"
#include <filesystem>
#include "Core.h"

bool Scene::Init(Name inName) noexcept
{
	Release();
	name = inName;
	return true;
}

void Scene::Release() noexcept
{

}

bool Scene::Load() noexcept
{
	std::filesystem::path path{ STR("Assets") };
	path /= name.ToString();

	std::ifstream stream{ path.string() };
	Json file;
	stream >> file;

	/// @todo: deserialization

	return true;
}

bool Scene::Save() noexcept
{
	std::filesystem::path path{ STR("Assets") };
	path /= name.ToString();
	
	Json file;

	/// @todo: serialization

	std::ofstream stream{ path.string() };
	stream << file;
	return true;
}
