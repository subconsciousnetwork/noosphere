import os

project_root = os.path.realpath(os.path.join(os.path.dirname(os.path.realpath(__file__)), '..'))

def get_project_root():
  return project_root
