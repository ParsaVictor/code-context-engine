"""Validation pass: load the checkpoint, score the model, report mAP."""

import torch
from torch.utils.data import DataLoader
from torchmetrics.detection import MeanAveragePrecision

from data.dataset import CocoDetection, build_transforms
from models.detector import Detector


def evaluate(model, loader):
    """Mean average precision over the validation split."""
    model.eval()
    metric = MeanAveragePrecision()
    with torch.no_grad():
        for images, targets in loader:
            metric.update(model(images), targets)
    result = metric.compute()
    logger.log("val/mAP", result["map"])
    return result


def main(image_size=640):
    pipeline = build_transforms(image_size=image_size, augment=False)
    dataset = CocoDetection("data/coco", split="val", transforms=pipeline)
    loader = DataLoader(dataset, batch_size=8, shuffle=False)

    model = Detector(num_classes=80)
    model.load_state_dict(torch.load("runs/last.pt"))
    return evaluate(model, loader)
