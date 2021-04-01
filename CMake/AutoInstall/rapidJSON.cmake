cmake_minimum_required (VERSION 3.10)
project (${NAME}-DOWNLOAD NONE)
include (ExternalProject)

ExternalProject_Add (
	${NAME}-DOWNLOAD
	GIT_REPOSITORY		https://github.com/Tencent/rapidjson/
	GIT_TAG				1.1.0
	GIT_SHALLOW			TRUE
	GIT_PROGRESS		TRUE
	SOURCE_DIR			"${src_dir}"
	BINARY_DIR			"${build_dir}"
	CONFIGURE_COMMAND	""
	BUILD_COMMAND		""
	INSTALL_COMMAND		""
	TEST_COMMAND		""
)
