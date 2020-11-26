#include "Scene.h"
#include <fstream>
#include <rapidjson/document.h>
#include <rapidjson/stringbuffer.h>
#include <rapidjson/prettywriter.h>
#include "Entity.h"
#include "IteratorFinder.h"
#include "JsonHelper.h"
#include "RenderManager.h"

namespace ArenaBoss
{
	namespace
	{
		rapidjson::Document LoadJson(const std::string& name)
		{
			std::ifstream file{ "Asset\\" + name, std::ios::in | std::ios::binary | std::ios::ate };
			if (!file.is_open())
				throw std::exception{ "File not found." };

			const auto size = file.tellg();
			file.seekg(0, std::ios::beg);

			std::string str;
			str.resize(size);
			file.read(str.data(), size);

			rapidjson::Document doc;
			doc.Parse(str.data());

			if (!doc.IsObject())
				throw std::exception{ "File is not vaild JSON." };

			return doc;
		}
	}

	void Scene::Init()
	{
		for (auto* entity : entities)
			entity->Init();
	}

	void Scene::Release() noexcept
	{
		for (auto entity : entities)
		{
			entity->Release();
			delete entity;
		}

		entities.clear();
	}

	void Scene::Load()
	{
		Release();

		const auto doc = LoadJson(name);

		const auto& globals = doc["globals"];

		if (!globals.IsObject())
			throw std::exception{ "File is not vaild." };

		auto& manager = Accessor<RenderManager>::GetManager();

		const auto ambient = Json::JsonHelper::GetVector3(globals, "ambientLight");
		if (ambient) manager.SetAmbientLight(*ambient);

		const auto& dirObj = globals["directionalLight"];
		if (dirObj.IsObject())
		{
			auto& dir = manager.GetDirectionalLight();

			dir.direction = *Json::JsonHelper::GetVector3(dirObj, "direction");
			dir.diffuseColor = *Json::JsonHelper::GetVector3(dirObj, "diffuseColor");
			dir.specularColor = *Json::JsonHelper::GetVector3(dirObj, "specularColor");
		}

		const auto& entitiesArray = doc["entities"];

		if (!entitiesArray.IsArray())
			throw std::exception{ "File is not vaild." };

		for (rapidjson::SizeType i = 0; i < entitiesArray.Size(); ++i)
		{
			const auto& entityObj = entitiesArray[i];
			if (!entityObj.IsObject()) continue;

			const auto name = Json::JsonHelper::GetString(entityObj, "name");
			if (!name) throw std::exception{ "Entity is not vaild." };

			auto* entity = AddEntity(*name);
			entity->Load(entityObj);
		}
	}

	void Scene::Load(const std::string& inName)
	{
		name = inName;
		Load();
	}

	void Scene::Load(std::string&& inName)
	{
		name = std::move(inName);
		Load();
	}

	void Scene::Save() const
	{
		rapidjson::Document doc;
		doc.SetObject();

		auto& alloc = doc.GetAllocator();

		rapidjson::Value globals{ rapidjson::kObjectType };
		auto& manager = Accessor<RenderManager>::GetManager();

		Json::JsonSaver saver{ alloc, globals };
		Json::JsonHelper::AddVector3(saver, "ambientLight", manager.GetAmbientLight());

		auto& dirObj = globals["directionalLight"];
		if (dirObj.IsObject())
		{
			auto& dir = manager.GetDirectionalLight();
			Json::JsonSaver dirSaver{ saver, dirObj };

			Json::JsonHelper::AddVector3(dirSaver, "direction", dir.direction);
			Json::JsonHelper::AddVector3(dirSaver, "diffuseColor", dir.diffuseColor);
			Json::JsonHelper::AddVector3(dirSaver, "specularColor", dir.specularColor);
		}

		doc.AddMember("globals", globals, alloc);

		rapidjson::Value entitiesArray{ rapidjson::kArrayType };
		
		for (const auto* entity : entities)
		{
			rapidjson::Value obj{ rapidjson::kObjectType };
			Json::JsonSaver saver{ alloc, obj };
			entity->Save(saver);
			entitiesArray.PushBack(obj, alloc);
		}

		doc.AddMember("entities", entitiesArray, alloc);

		rapidjson::StringBuffer buffer;
		rapidjson::PrettyWriter<rapidjson::StringBuffer> writer{ buffer };
		doc.Accept(writer);

		std::ofstream outFile{ name, std::ios::trunc };
		if (outFile.is_open())
			outFile << buffer.GetString();
	}

	void Scene::Save(const std::string& inName)
	{
		name = inName;
		Save();
	}

	void Scene::Save(std::string&& inName)
	{
		name = std::move(inName);
		Save();
	}

	Entity* Scene::AddEntity(const std::string& inName)
	{
		return AddEntity(new Entity{ inName });
	}

	Entity* Scene::AddEntity(Entity* entity)
	{
		const auto iter = std::upper_bound(
			entities.cbegin(),
			entities.cend(),
			entity->GetName(),
			[](const auto& lhs, const auto& rhs) { return lhs < *rhs; }
		);
		entities.insert(iter, entity);
		return entity;
	}

	void Scene::RemoveEntity(const std::string& inName)
	{
		const auto iter = IteratorFinder::FindSameIterator(entities, inName);
		delete *iter;
		entities.erase(iter);
	}

	void Scene::RemoveEntity(Entity* entity)
	{
		RemoveEntity(entity->GetName());
	}

	Entity* Scene::GetEntity(const std::string& inName)
	{
		const auto iter = IteratorFinder::FindSameIterator(entities, inName);
		return *iter;
	}
}