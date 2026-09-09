"""Single-stage detector: backbone, head, and the detection loss."""

import torch
import torch.nn as nn


class Detector(nn.Module):
    """The model trained by engine/train.py and scored by engine/evaluate.py."""

    def __init__(self, num_classes=80, width=64):
        super().__init__()
        self.backbone = nn.Sequential(
            nn.Conv2d(3, width, kernel_size=3, padding=1),
            nn.ReLU(inplace=True),
        )
        self.head = nn.Linear(width, num_classes * 5)
        self.num_classes = num_classes

    def forward(self, images):
        features = self.backbone(images)
        return self.head(features.mean(dim=(2, 3)))


def detection_loss(predictions, targets):
    """Classification plus box regression."""
    return torch.nn.functional.mse_loss(predictions, targets)
