# DownloadExternal.py
from threading import Thread
from pathlib import Path
from requests import get
import tarfile
import shutil
import os

def get_config_path():
    return Path.cwd().parent / 'Config' / 'ExternalPath.ini'

def get_cmake_path():
    return Path.cwd().parent / 'CMake' / 'Package.cmake'

def make_abs_path(*names):
    ret = Path.cwd().parent / 'External'
    for name in names:
        ret /= name
    return ret

def find_lines(name, contents, is_comment):
    nums = []
    for num, line in enumerate(contents):
        if name in line and (line[0] == '#') == is_comment:
            nums.append(num)
    return nums

def set_cmake(name, action, is_comment):
    contents = get_cmake_path().read_text().splitlines()
    lines = find_lines(name, contents, is_comment)

    for line in lines:
        contents[line] = action(contents[line])

    get_cmake_path().write_text("\n".join(contents))

def add_cmake(name):
    set_cmake(name, lambda content: content[2:], True)

def del_cmake(name):
    set_cmake(name, lambda content: '# ' + content, False)

def init_folder():
    path = make_abs_path()
    if path.exists():
        shutil.rmtree(path)
    path.mkdir()

def init_cmake():
    del_cmake('find_package')

def init():
    init_folder()
    init_cmake()

def get_config():
    config = {}
    for line in get_config_path().read_text().splitlines():
        name, url = line.split('=')
        config[name] = url
    return config

def set_config(config):
    with get_config_path().open('wt') as file:
        for pair in config.items():
            file.write(f'{pair[0]}={pair[1]}')

def register(name, url):
    config = get_config()
    config[name] = url
    set_config(config)
    print(f'Successfully registered {name}')

def unregister(name):
    config = get_config()
    del config[name]
    set_config(config)
    print(f'Successfully unregistered {name}')

def search(name):
    return make_abs_path(name).exists()

def print_search(name):
    if (search(name)):
        print(f'{name} already installed.')
    else:
        print(f'{name} is not installed.')

def add_package(name, url):
    print(f'Start downloading {name}.')
    path = make_abs_path(Path(url).name)
    path.write_bytes(get(url).content)

    print(f'Extracting {name}.')
    with tarfile.open(path) as file:
        file.extractall(make_abs_path())

    path.unlink()
    folder_path = Path(str(path)[:-7])
    folder_path.rename(make_abs_path(name))

def del_package(name):
    path = make_abs_path(name)
    shutil.rmtree(path, ignore_errors=True)

def install_single(name):
    if not search(name):
        add_package(name, get_config()[name])
        add_cmake(name)
        print(f'{name} has been added successfully.')
    else:
        print(f'{name} already installed.')

def install_all():
    init()
    threads = []

    for name in get_config().keys():
        thread = Thread(target=install_single, args=[name])
        thread.start()
        threads.append(thread)

    for thread in threads:
        thread.join()

def install(name):
    if (name == 'all'):
        install_all()
    else:
        install_single(name)

def uninstall(name):
    if search(name):
        del_package(name)
        del_cmake(name)
        print(f'Successfully deleted {name}.')
    else:
        print(f'{name} is not installed.')