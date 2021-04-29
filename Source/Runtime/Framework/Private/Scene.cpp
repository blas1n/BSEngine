#include "Scene.h"
#include <filesystem>
#include "Core.h"

void Scene::Init(Name inName)
{
	name = inName;
}

void Scene::Release() noexcept
{

}

void Scene::Load()
{
	std::filesystem::path path{ STR("Assets") };
	path /= name.ToString();

	std::ifstream stream{ path.string() };
	Json file;
	stream >> file;

	/// @todo: deserialization
}

void Scene::Save()
{
	std::filesystem::path path{ STR("Assets") };
	path /= name.ToString();
	
	Json file;

	/// @todo: serialization

	std::ofstream stream{ path.string() };
	stream << file;
}
